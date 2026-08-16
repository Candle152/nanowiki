//! File extraction — compress each source file into a structured summary.
//!
//! Each file is processed independently (no shared context), so this phase
//! can handle arbitrarily large repositories without context overflow.

use std::path::Path;

use crate::agent::{AgentResponse, Client};
use rig_core::completion::request::CompletionRequest;
use rig_core::OneOrMany;

const EXTRACT_SYSTEM_PROMPT: &str = "\
Extract a structured summary from this source file. Output compact Markdown.

Format:
```
# {relative_path}

## Public API
- `pub fn name(...)` — one-line description
- `pub struct Name` — one-line description
(omit if file has no public API)

## Imports / Dependencies
- module::path — what it's used for
(key external dependencies only, skip stdlib)

## Purpose
1-2 sentences: what this file does and its role in the project.

## Key Details
3-5 bullets of the most important implementation details, patterns, or invariants.
```

Rules:
- Be concise. Prefer 5-10 lines total for a typical file.
- Skip trivial getters, constants, and test helpers.
- If the file is a config/manifest, list the key settings.
- If the file is mostly comments/boilerplate, say so briefly.
- Write in English.
- Output ONLY the Markdown, no preamble.";

// ── Skip lists: add new entries here to filter more files ──

/// Directory names that are never extracted.
const SKIP_DIRS: &[&str] = &[
    "node_modules", "target", "vendor", "dist", "build",
    "__pycache__", "venv", "coverage", "out",
];

/// File names that are never extracted.
const SKIP_NAMES: &[&str] = &[
    // Lock files
    "Cargo.lock", "package-lock.json", "yarn.lock", "pnpm-lock.yaml",
    // OS
    ".DS_Store", "Thumbs.db",
    // Sensitive
    ".env",
    // Legal / license
    "LICENSE", "LICENSE.md", "LICENSE.txt", "COPYING",
    // Agent instructions (generated)
    "AGENTS.md", "CLAUDE.md",
];

/// File extensions that are never extracted.
const SKIP_EXTENSIONS: &[&str] = &[
    // Images
    "png", "jpg", "jpeg", "gif", "bmp", "ico", "webp", "svg",
    // Fonts
    "ttf", "otf", "woff", "woff2", "eot",
    // Binaries
    "exe", "dll", "so", "dylib", "o", "obj", "a", "lib",
    // Archives
    "zip", "tar", "gz", "bz2", "xz", "7z", "rar",
    // Media
    "mp3", "wav", "ogg", "mp4", "avi", "mov",
    // Documents
    "pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx",
    // Other
    "wasm", "bin", "dat", "db", "sqlite",
];

/// Check whether a file should be extracted (not skipped).
pub fn should_extract(path: &Path, size: u64) -> bool {
    // Skip large files
    if size > 100 * 1024 || size == 0 {
        return false;
    }

    // Skip dot-directories and build/output dirs
    for component in path.components() {
        if let Some(s) = component.as_os_str().to_str() {
            if s.starts_with('.') || SKIP_DIRS.contains(&s) {
                return false;
            }
        }
    }

    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };

    // Skip sensitive names: .env, lock files, and names containing secret/token/key/credential
    let lower = name.to_lowercase();
    if SKIP_NAMES.contains(&name)
        || lower.contains("secret")
        || lower.contains("token")
        || lower.contains("key")
        || lower.contains("credential")
    {
        return false;
    }

    // Skip binary/non-code extensions
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        if SKIP_EXTENSIONS.contains(&ext.to_lowercase().as_str()) {
            return false;
        }
    }

    true
}

/// Return the path where the summary for `file_path` would be stored.
pub fn summary_path(scan_dir: &Path, file_path: &str) -> std::path::PathBuf {
    let rel_dir = Path::new(file_path).parent().unwrap_or(Path::new(""));
    let stem = Path::new(file_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");
    let ext = Path::new(file_path)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    let out_name = if ext.is_empty() {
        format!("{}.summary.md", stem)
    } else {
        format!("{}.{}.summary.md", stem, ext)
    };
    scan_dir.join(rel_dir).join(out_name)
}

/// Extract structured summary from a single file.
///
/// Reads the file, sends it to the LLM with EXTRACT_SYSTEM_PROMPT,
/// returns (prompt_tokens, completion_tokens) on success.
pub async fn extract_file(
    client: &Client,
    model: &str,
    repo_root: &Path,
    file_path: &str,
    scan_dir: &Path,
) -> anyhow::Result<(u32, u32)> {
    let full_path = repo_root.join(file_path);

    // Read file content
    let content = match std::fs::read_to_string(&full_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("  skip {}: cannot read ({})", file_path, e);
            return Ok((0, 0));
        }
    };

    // Truncate large files for extraction (100KB limit for the prompt)
    let content = if content.len() > 100 * 1024 {
        let truncated = String::from_utf8_lossy(
            crate::tools::truncate_utf8(content.as_bytes(), 100 * 1024)
        );
        format!("{}... [truncated at 100KB]", truncated)
    } else {
        content
    };

    let user_msg = format!("File: {}\n\n```\n{}\n```", file_path, content);

    let request = CompletionRequest {
        model: Some(model.to_string()),
        chat_history: OneOrMany::many(vec![
            rig_core::completion::Message::system(EXTRACT_SYSTEM_PROMPT),
            rig_core::completion::Message::user(&user_msg),
        ])
        .expect("chat_history must not be empty"),
        preamble: None,
        tools: vec![],
        tool_choice: None,
        max_tokens: Some(2048),
        temperature: Some(0.1),
        additional_params: None,
        documents: vec![],
        output_schema: None,
        record_telemetry_content: false,
    };

    let response: AgentResponse = client.completion(request).await?;

    let summary = response.text.unwrap_or_else(|| "(no output)".to_string());

    let usage = response.usage.unwrap_or(crate::agent::UsageInfo {
        prompt_tokens: 0,
        completion_tokens: 0,
    });

    // Write to _scan/ directory, mirroring the original path structure
    let out_path = summary_path(scan_dir, file_path);
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&out_path, &summary)?;

    Ok((usage.prompt_tokens, usage.completion_tokens))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skips_lock_files() {
        assert!(!should_extract(Path::new("Cargo.lock"), 100));
        assert!(!should_extract(Path::new("yarn.lock"), 100));
    }

    #[test]
    fn skips_binary_extensions() {
        assert!(!should_extract(Path::new("icon.png"), 100));
        assert!(!should_extract(Path::new("app.exe"), 100));
        assert!(!should_extract(Path::new("font.ttf"), 100));
    }

    #[test]
    fn skips_dot_directories() {
        assert!(!should_extract(Path::new(".git/config"), 100));
        assert!(!should_extract(Path::new("src/.hidden/file.rs"), 100));
        assert!(!should_extract(Path::new(".github/workflows/ci.yml"), 100));
        assert!(!should_extract(Path::new(".nanowiki/INDEX.md"), 100));
    }

    #[test]
    fn skips_build_directories() {
        assert!(!should_extract(Path::new("node_modules/pkg/file.js"), 100));
        assert!(!should_extract(Path::new("target/debug/main.rs"), 100));
        assert!(!should_extract(Path::new("dist/bundle.js"), 100));
    }

    #[test]
    fn skips_large_files() {
        assert!(!should_extract(Path::new("big.rs"), 200 * 1024));
        assert!(should_extract(Path::new("small.rs"), 50 * 1024));
    }

    #[test]
    fn skips_empty_files() {
        assert!(!should_extract(Path::new("empty.rs"), 0));
    }

    #[test]
    fn skips_sensitive_filenames() {
        assert!(!should_extract(Path::new(".env"), 100));
        assert!(!should_extract(Path::new("api_token.txt"), 100));
        assert!(!should_extract(Path::new("secret_key.rs"), 100));
        assert!(!should_extract(Path::new("credentials.json"), 100));
    }

    #[test]
    fn accepts_code_files() {
        assert!(should_extract(Path::new("src/main.rs"), 5000));
        assert!(should_extract(Path::new("lib.rs"), 2000));
        assert!(should_extract(Path::new("Cargo.toml"), 500));
        assert!(should_extract(Path::new("README.md"), 1000));
    }
}
