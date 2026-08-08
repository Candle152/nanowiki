//! SearchCode tool — search text across repository files

use std::path::Path;

#[derive(Debug, PartialEq)]
pub struct SearchMatch {
    pub file_path: String,
    pub line_number: usize,
    pub line_content: String,
}

/// Search for a plain-text substring across repository files (case-sensitive).
///
/// Skips `.git` directories, `.gitignore`-excluded files, and binary files.
/// Returns up to 100 matches.
pub fn search_code(
    repo_root: &Path,
    pattern: &str,
    search_path: Option<&str>,
) -> Result<Vec<SearchMatch>, String> {
    if pattern.is_empty() {
        return Err("search pattern cannot be empty".into());
    }

    let walk_root = match search_path {
        Some(p) => {
            crate::tools::validate_relative_path(p)?;
            let resolved = repo_root.join(Path::new(p));
            if !resolved.is_dir() {
                return Err(format!("path not found or not a directory: {}", p));
            }
            resolved
        }
        None => repo_root.to_path_buf(),
    };

    let walker = ignore::WalkBuilder::new(&walk_root)
        .require_git(false)
        .build()
        .filter_map(|r| r.ok())
        .filter(|e| e.file_type().is_some_and(|t| t.is_file()));

    let mut results: Vec<SearchMatch> = Vec::new();
    const MAX_MATCHES: usize = 100;

    for entry in walker {
        let path = entry.path();
        if is_binary_by_extension(path) {
            continue;
        }

        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue, // skip unreadable files
        };

        let rel_path = path
            .strip_prefix(repo_root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");

        for (i, line) in content.lines().enumerate() {
            if line.contains(pattern) {
                if results.len() >= MAX_MATCHES {
                    return Ok(results);
                }
                results.push(SearchMatch {
                    file_path: rel_path.clone(),
                    line_number: i + 1, // 1-indexed
                    line_content: line.to_string(),
                });
            }
        }
    }

    Ok(results)
}

fn is_binary_by_extension(path: &Path) -> bool {
    const BINARY_EXTENSIONS: &[&str] = &[
        "exe", "dll", "so", "dylib", "o", "obj", "a", "lib",
        "png", "jpg", "jpeg", "gif", "bmp", "ico", "webp",
        "mp3", "wav", "ogg", "mp4", "avi", "mov",
        "zip", "tar", "gz", "bz2", "xz", "7z", "rar",
        "pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx",
        "ttf", "otf", "woff", "woff2", "eot",
        "wasm", "bin", "dat", "db", "sqlite",
    ];
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| BINARY_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn setup(root: &Path) {
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/main.rs"), "fn main() {\n    println!(\"hello\");\n}\n").unwrap();
        fs::write(
            root.join("src/lib.rs"),
            "pub fn greet() -> String {\n    \"hello\".to_string()\n}\n",
        )
        .unwrap();
        fs::write(root.join("README.md"), "# My Project\n## hello\n").unwrap();
    }

    #[test]
    fn finds_text_in_files() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        setup(root);

        let matches = search_code(root, "hello", None).unwrap();
        let files: Vec<&str> = matches.iter().map(|m| m.file_path.as_str()).collect();
        assert!(files.contains(&"src/main.rs"));
        assert!(files.contains(&"src/lib.rs"));
        assert!(files.contains(&"README.md"));
    }

    #[test]
    fn reports_line_numbers() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        setup(root);

        let matches = search_code(root, "hello", None).unwrap();
        let main_match = matches.iter().find(|m| m.file_path == "src/main.rs").unwrap();
        assert_eq!(main_match.line_number, 2); // 1-indexed
    }

    #[test]
    fn respects_search_path() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        setup(root);

        let matches = search_code(root, "hello", Some("src")).unwrap();
        let files: Vec<&str> = matches.iter().map(|m| m.file_path.as_str()).collect();
        assert!(files.contains(&"src/main.rs"));
        assert!(!files.contains(&"README.md"), "README is not under src/");
    }

    #[test]
    fn rejects_empty_pattern() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();

        let result = search_code(root, "", None);
        assert!(result.is_err(), "empty pattern should be rejected");
    }

    #[test]
    fn empty_result_for_no_match() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        setup(root);

        let matches = search_code(root, "nonexistentXYZ123", None).unwrap();
        assert!(matches.is_empty());
    }

    #[test]
    fn rejects_parent_dir_in_search_path() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        setup(root);

        let result = search_code(root, "hello", Some("../etc"));
        assert!(result.is_err(), ".. path should be rejected");
    }

    #[test]
    fn rejects_absolute_search_path() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let abs = if cfg!(windows) { "C:\\etc" } else { "/etc" };

        let result = search_code(root, "hello", Some(abs));
        assert!(result.is_err(), "absolute path should be rejected");
    }
}
