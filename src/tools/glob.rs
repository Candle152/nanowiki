//! GlobFiles tool — match file paths against a glob pattern

use std::path::Path;

/// Match file paths in the repository against a glob pattern.
///
/// Respects `.gitignore`. Returns up to 500 matches.
pub fn glob_files(repo_root: &Path, pattern: &str) -> Result<Vec<String>, String> {
    let walker = ignore::WalkBuilder::new(repo_root)
        .require_git(false)
        .build()
        .filter_map(|r| r.ok())
        .filter(|e| e.file_type().is_some_and(|t| t.is_file()));

    let glob = globset::Glob::new(pattern)
        .map_err(|e| format!("invalid glob pattern: {}", e))?
        .compile_matcher();

    let mut results: Vec<String> = Vec::new();
    for entry in walker {
        let rel_path = entry
            .path()
            .strip_prefix(repo_root)
            .map_err(|e| format!("path resolution error: {}", e))?;
        let path_str = rel_path.to_string_lossy().replace('\\', "/");

        if glob.is_match(&path_str) {
            if results.len() >= 500 {
                break;
            }
            results.push(path_str);
        }
    }

    results.sort();
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn setup(repo_root: &Path) {
        fs::create_dir_all(repo_root.join("src/api")).unwrap();
        fs::create_dir_all(repo_root.join("tests")).unwrap();
        fs::write(repo_root.join("src/main.rs"), "").unwrap();
        fs::write(repo_root.join("src/lib.rs"), "").unwrap();
        fs::write(repo_root.join("src/api/auth.rs"), "").unwrap();
        fs::write(repo_root.join("src/api/mod.rs"), "").unwrap();
        fs::write(repo_root.join("tests/auth_test.rs"), "").unwrap();
        fs::write(repo_root.join("Cargo.toml"), "").unwrap();
    }

    #[test]
    fn matches_recursive_glob() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        setup(root);

        let files = glob_files(root, "src/**/*.rs").unwrap();
        let mut files = files;
        files.sort();
        assert_eq!(
            files,
            vec![
                "src/api/auth.rs",
                "src/api/mod.rs",
                "src/lib.rs",
                "src/main.rs",
            ]
        );
    }

    #[test]
    fn matches_single_level_glob() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        setup(root);

        let files = glob_files(root, "*.toml").unwrap();
        assert_eq!(files, vec!["Cargo.toml"]);
    }

    #[test]
    fn respects_gitignore() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        setup(root);
        fs::write(root.join(".gitignore"), "*.log\n").unwrap();
        fs::write(root.join("debug.log"), "").unwrap();
        fs::write(root.join("src/app.log"), "").unwrap();

        let files = glob_files(root, "**/*.log").unwrap();
        assert!(files.is_empty(), ".gitignore'd files should not match");
    }

    #[test]
    fn returns_relative_paths() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        setup(root);

        let files = glob_files(root, "tests/*.rs").unwrap();
        assert_eq!(files, vec!["tests/auth_test.rs"]);
    }

    #[test]
    fn empty_result_for_no_match() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();

        let files = glob_files(root, "nonexistent/*.rs").unwrap();
        assert!(files.is_empty());
    }
}
