//! File scanner — walk the repository tree and produce a file list

use std::path::Path;

#[derive(Debug)]
pub struct FileEntry {
    pub relative_path: String,
}

const LOCK_FILES: &[&str] = &[
    "Cargo.lock", "pnpm-lock.yaml", "yarn.lock", "package-lock.json",
    "Gemfile.lock", "poetry.lock", "mix.lock",
];

/// Scan the repository and return a list of all code files.
pub fn scan_repo(repo_root: &Path) -> Result<Vec<FileEntry>, String> {
    let walker = ignore::WalkBuilder::new(repo_root)
        .require_git(false)
        .build()
        .filter_map(|r| r.ok())
        .filter(|e| e.file_type().is_some_and(|t| t.is_file()));

    let mut entries: Vec<FileEntry> = Vec::new();

    for entry in walker {
        let path = entry.path();

        if let Some(name) = path.file_name().and_then(|n| n.to_str())
            && LOCK_FILES.contains(&name) {
                continue;
            }

        let rel = path
            .strip_prefix(repo_root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");

        entries.push(FileEntry {
            relative_path: rel,
        });
    }

    entries.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn lists_all_code_files() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/main.rs"), "fn main() {}").unwrap();
        fs::write(root.join("Cargo.toml"), "[package]").unwrap();
        fs::write(root.join("README.md"), "# Readme").unwrap();

        let entries = scan_repo(root).unwrap();
        let paths: Vec<&str> = entries.iter().map(|e| e.relative_path.as_str()).collect();
        assert!(paths.contains(&"src/main.rs"));
        assert!(paths.contains(&"Cargo.toml"));
        assert!(paths.contains(&"README.md"));
    }

    #[test]
    fn filters_gitignore() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::write(root.join(".gitignore"), "*.log\n").unwrap();
        fs::write(root.join("main.rs"), "fn main() {}").unwrap();
        fs::write(root.join("debug.log"), "log").unwrap();

        let entries = scan_repo(root).unwrap();
        let paths: Vec<&str> = entries.iter().map(|e| e.relative_path.as_str()).collect();
        assert!(paths.contains(&"main.rs"));
        assert!(!paths.contains(&"debug.log"));
    }

    #[test]
    fn filters_lock_files() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::write(root.join("main.rs"), "fn main() {}").unwrap();
        fs::write(root.join("Cargo.lock"), "lock").unwrap();

        let entries = scan_repo(root).unwrap();
        let names: Vec<&str> = entries.iter().map(|e| e.relative_path.as_str()).collect();
        assert!(names.contains(&"main.rs"));
        assert!(!names.contains(&"Cargo.lock"));
    }
}
