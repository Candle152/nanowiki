//! ReadFile tool — read a single file from the repository

use std::path::Path;

fn is_ignored_by_gitignore(repo_root: &Path, full_path: &Path) -> bool {
    let parent = full_path.parent().unwrap_or(repo_root);
    let file_name = match full_path.file_name() {
        Some(n) => n,
        None => return false,
    };

    let walker = ignore::WalkBuilder::new(parent)
        .require_git(false)
        .max_depth(Some(1))
        .build();

    for entry in walker.filter_map(|r| r.ok()) {
        if entry.depth() == 1 && entry.file_name() == file_name {
            return false; // file is in walk output — not ignored
        }
    }
    true // not in walk output — filtered by .gitignore
}

/// Read a single file from the repository.
///
/// UTF-8 text files only, truncated to 50KB.
/// Rejects sensitive files (.env, names containing secret/token/key/credential).
///
/// `offset` and `limit` control line-range reading (1-indexed):
/// - Both None: read the entire file (with 50KB truncation).
/// - offset=Some(N): start at line N.
/// - limit=Some(M): read at most M lines.
pub fn read_file(
    repo_root: &Path,
    file_path: &str,
    offset: Option<usize>,
    limit: Option<usize>,
) -> Result<String, String> {
    crate::tools::validate_relative_path(file_path)?;

    let full_path = repo_root.join(Path::new(file_path));

    if !full_path.is_file() {
        if full_path.is_dir() {
            return Err("is a directory, use ListDirectory instead".into());
        }
        return Err(format!("file not found: {}", file_path));
    }

    if is_ignored_by_gitignore(repo_root, &full_path) {
        return Err("file is ignored by .gitignore".into());
    }

    let name = full_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    let lower = name.to_lowercase();
    if name == ".env"
        || lower.contains("secret")
        || lower.contains("token")
        || lower.contains("key")
        || lower.contains("credential")
    {
        return Err("refusing to read sensitive file (name contains secret/token/key/credential or is .env)".into());
    }

    const MAX_SIZE: u64 = 50 * 1024; // 50KB
    let metadata = std::fs::metadata(&full_path).map_err(|e| format!("cannot stat file: {}", e))?;
    let content = std::fs::read_to_string(&full_path).map_err(|e| format!("cannot read file: {}", e))?;

    // Line-range mode
    if offset.is_some() || limit.is_some() {
        return Ok(read_with_line_range(file_path, &content, offset, limit));
    }

    // Full-file mode (original behavior)
    if metadata.len() <= MAX_SIZE {
        Ok(content)
    } else {
        // truncate at 50KB boundary, avoid splitting a UTF-8 character
        let truncated_bytes = crate::tools::truncate_utf8(content.as_bytes(), MAX_SIZE as usize);
        let truncated = String::from_utf8_lossy(truncated_bytes);
        Ok(format!("{}\n... [truncated at 50KB]", truncated))
    }
}

fn read_with_line_range(
    file_path: &str,
    content: &str,
    offset: Option<usize>,
    limit: Option<usize>,
) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let total = lines.len();
    let start_line = offset.unwrap_or(1);
    let start_idx = start_line.saturating_sub(1);

    if start_idx >= total {
        return format!(
            "[{}] (offset {} exceeds file of {} lines)",
            file_path, start_line, total
        );
    }

    let end_idx = match limit {
        Some(n) => std::cmp::min(start_idx + n, total),
        None => total,
    };

    let range_lines = &lines[start_idx..end_idx];
    let end_line = start_line + range_lines.len() - 1;
    let remaining = if end_idx < total {
        format!(" (+{} more lines)", total - end_idx)
    } else {
        String::new()
    };
    let header = format!("[{}, lines {}-{}{}]", file_path, start_line, end_line, remaining);

    let body = range_lines
        .iter()
        .enumerate()
        .map(|(i, line)| format!("{:>4}: {}", start_line + i, line))
        .collect::<Vec<_>>()
        .join("\n");

    format!("{}\n{}", header, body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn reads_text_file() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::write(root.join("main.rs"), "fn main() {}").unwrap();

        let content = read_file(root, "main.rs", None, None).unwrap();
        assert_eq!(content, "fn main() {}");
    }

    #[test]
    fn reads_line_range() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::write(root.join("lib.rs"), "// top\npub fn a() {}\npub fn b() {}\npub fn c() {}\n// bottom\n").unwrap();

        let content = read_file(root, "lib.rs", Some(2), Some(3)).unwrap();
        assert!(content.contains("[lib.rs, lines 2-4 (+1 more lines)]"));
        assert!(content.contains("   2: pub fn a() {}"));
        assert!(content.contains("   4: pub fn c() {}"));
        assert!(!content.contains("// top"));
        assert!(!content.contains("// bottom"));
    }

    #[test]
    fn reads_with_offset_only() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::write(root.join("lib.rs"), "line1\nline2\nline3\n").unwrap();

        let content = read_file(root, "lib.rs", Some(2), None).unwrap();
        assert!(content.contains("[lib.rs, lines 2-3]"));
        assert!(!content.contains("line1"));
        assert!(content.contains("line2"));
        assert!(content.contains("line3"));
    }

    #[test]
    fn reads_with_limit_only() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::write(root.join("lib.rs"), "line1\nline2\nline3\n").unwrap();

        let content = read_file(root, "lib.rs", None, Some(2)).unwrap();
        assert!(content.contains("[lib.rs, lines 1-2 (+1 more lines)]"));
        assert!(content.contains("line1"));
        assert!(content.contains("line2"));
        assert!(!content.contains("line3"));
    }

    #[test]
    fn offset_exceeds_file_shows_error() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::write(root.join("lib.rs"), "one line\n").unwrap();

        let content = read_file(root, "lib.rs", Some(10), None).unwrap();
        assert!(content.contains("offset 10 exceeds file of 1 lines"));
    }

    #[test]
    fn reads_with_all_parameters() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let mut lines = String::new();
        for i in 1..=20 {
            lines.push_str(&format!("line{}\n", i));
        }
        fs::write(root.join("big.rs"), &lines).unwrap();

        let content = read_file(root, "big.rs", Some(5), Some(5)).unwrap();
        assert!(content.contains("[big.rs, lines 5-9 (+11 more lines)]"));
        assert!(!content.contains("line4"));
        assert!(content.contains("line5"));
        assert!(content.contains("line9"));
        assert!(!content.contains("line10"));
    }

    #[test]
    fn rejects_parent_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();

        let result = read_file(root, "../secret.txt", None, None);
        assert!(result.is_err());
    }

    #[test]
    fn rejects_absolute_path() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let abs = if cfg!(windows) { "C:\\etc\\passwd" } else { "/etc/passwd" };

        let result = read_file(root, abs, None, None);
        assert!(result.is_err());
    }

    #[test]
    fn rejects_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join("mydir")).unwrap();

        let result = read_file(root, "mydir", None, None);
        assert!(result.is_err());
    }

    #[test]
    fn rejects_sensitive_files() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::write(root.join(".env"), "SECRET=123").unwrap();

        let result = read_file(root, ".env", None, None);
        assert!(result.is_err(), ".env should be rejected");
    }

    #[test]
    fn rejects_files_with_token_in_name() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::write(root.join("api_token.txt"), "abc").unwrap();

        let result = read_file(root, "api_token.txt", None, None);
        assert!(result.is_err(), "filenames containing 'token' should be rejected");
    }

    #[test]
    fn truncates_large_file() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let big = "A".repeat(60 * 1024);
        fs::write(root.join("big.txt"), &big).unwrap();

        let content = read_file(root, "big.txt", None, None).unwrap();
        assert!(content.len() <= 50 * 1024 + 100, "should truncate to ~50KB");
        assert!(content.ends_with("50KB]"));
    }

    #[test]
    fn error_on_nonexistent_file() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();

        let result = read_file(root, "nope.txt", None, None);
        assert!(result.is_err());
    }

    #[test]
    fn rejects_gitignored_file() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::write(root.join(".gitignore"), "*.log\n").unwrap();
        fs::write(root.join("debug.log"), "secret").unwrap();
        fs::write(root.join("main.rs"), "fn main() {}").unwrap();

        assert!(read_file(root, "debug.log", None, None).is_err(), ".gitignore'd file should be rejected");
        assert!(read_file(root, "main.rs", None, None).is_ok(), "non-ignored file should be readable");
    }
}
