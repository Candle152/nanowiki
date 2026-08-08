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
pub fn read_file(repo_root: &Path, file_path: &str) -> Result<String, String> {
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

    if metadata.len() <= MAX_SIZE {
        Ok(content)
    } else {
        // truncate at 50KB boundary, avoid splitting a UTF-8 character
        let bytes = content.as_bytes();
        let mut end = MAX_SIZE as usize;
        while end > 0 && (bytes[end] & 0xC0) == 0x80 {
            end -= 1; // back up to a complete UTF-8 character boundary
        }
        let truncated = String::from_utf8_lossy(&bytes[..end]);
        Ok(format!("{}\n... [truncated at 50KB]", truncated))
    }
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

        let content = read_file(root, "main.rs").unwrap();
        assert_eq!(content, "fn main() {}");
    }

    #[test]
    fn rejects_parent_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();

        let result = read_file(root, "../secret.txt");
        assert!(result.is_err());
    }

    #[test]
    fn rejects_absolute_path() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let abs = if cfg!(windows) { "C:\\etc\\passwd" } else { "/etc/passwd" };

        let result = read_file(root, abs);
        assert!(result.is_err());
    }

    #[test]
    fn rejects_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join("mydir")).unwrap();

        let result = read_file(root, "mydir");
        assert!(result.is_err());
    }

    #[test]
    fn rejects_sensitive_files() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::write(root.join(".env"), "SECRET=123").unwrap();

        let result = read_file(root, ".env");
        assert!(result.is_err(), ".env should be rejected");
    }

    #[test]
    fn rejects_files_with_token_in_name() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::write(root.join("api_token.txt"), "abc").unwrap();

        let result = read_file(root, "api_token.txt");
        assert!(result.is_err(), "filenames containing 'token' should be rejected");
    }

    #[test]
    fn truncates_large_file() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let big = "A".repeat(60 * 1024);
        fs::write(root.join("big.txt"), &big).unwrap();

        let content = read_file(root, "big.txt").unwrap();
        assert!(content.len() <= 50 * 1024 + 100, "should truncate to ~50KB");
        assert!(content.ends_with("50KB]"));
    }

    #[test]
    fn error_on_nonexistent_file() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();

        let result = read_file(root, "nope.txt");
        assert!(result.is_err());
    }

    #[test]
    fn rejects_gitignored_file() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::write(root.join(".gitignore"), "*.log\n").unwrap();
        fs::write(root.join("debug.log"), "secret").unwrap();
        fs::write(root.join("main.rs"), "fn main() {}").unwrap();

        assert!(read_file(root, "debug.log").is_err(), ".gitignore'd file should be rejected");
        assert!(read_file(root, "main.rs").is_ok(), "non-ignored file should be readable");
    }
}
