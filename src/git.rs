//! Git operations — commit hash / diff / changed files

use std::path::Path;

#[derive(Debug, PartialEq)]
pub struct ChangedFile {
    pub path: String,
    pub status: ChangeStatus,
}

#[derive(Debug, PartialEq)]
pub enum ChangeStatus {
    Added,
    Modified,
    Deleted,
}

/// Get HEAD commit hash (full 40-char SHA)
pub fn get_head_commit(repo_root: &Path) -> Result<String, String> {
    let repo = git2::Repository::open(repo_root)
        .map_err(|e| format!("cannot open git repository: {}", e))?;
    let head = repo.head().map_err(|e| format!("cannot get HEAD: {}", e))?;
    let commit = head.peel_to_commit().map_err(|e| format!("cannot get commit: {}", e))?;
    Ok(commit.id().to_string())
}

/// Get list of files changed between two commits
pub fn get_changed_files(
    repo_root: &Path,
    since_commit: &str,
) -> Result<Vec<ChangedFile>, String> {
    let repo = git2::Repository::open(repo_root)
        .map_err(|e| format!("cannot open git repository: {}", e))?;
    let since_oid = git2::Oid::from_str(since_commit)
        .map_err(|e| format!("invalid commit hash: {}", e))?;
    let since_commit = repo.find_commit(since_oid)
        .map_err(|e| format!("commit not found: {}", e))?;
    let since_tree = since_commit.tree().map_err(|e| format!("cannot get tree: {}", e))?;

    let head = repo.head().map_err(|e| format!("cannot get HEAD: {}", e))?;
    let head_commit = head.peel_to_commit().map_err(|e| format!("cannot get commit: {}", e))?;
    let head_tree = head_commit.tree().map_err(|e| format!("cannot get tree: {}", e))?;

    let diff = repo
        .diff_tree_to_tree(Some(&since_tree), Some(&head_tree), None)
        .map_err(|e| format!("cannot compute diff: {}", e))?;

    let mut files: Vec<ChangedFile> = Vec::new();
    diff.foreach(
        &mut |delta, _| {
            let path = delta
                .new_file()
                .path()
                .or_else(|| delta.old_file().path());
            if let Some(path) = path {
                let status = match delta.status() {
                    git2::Delta::Added => ChangeStatus::Added,
                    git2::Delta::Deleted => ChangeStatus::Deleted,
                    _ => ChangeStatus::Modified,
                };
                files.push(ChangedFile {
                    path: path.to_string_lossy().replace('\\', "/"),
                    status,
                });
            }
            true
        },
        None,
        None,
        None,
    )
    .map_err(|e| format!("diff iteration failed: {}", e))?;

    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn init_git_repo(root: &Path) {
        Command::new("git")
            .args(["init"])
            .current_dir(root)
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(root)
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(root)
            .output()
            .unwrap();
    }

    fn git_commit_all(root: &Path, msg: &str) {
        Command::new("git")
            .args(["add", "-A"])
            .current_dir(root)
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", msg])
            .current_dir(root)
            .output()
            .unwrap();
    }

    #[test]
    fn gets_head_commit() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        init_git_repo(root);
        std::fs::write(root.join("file.txt"), "hello").unwrap();
        git_commit_all(root, "initial");

        let head = get_head_commit(root).unwrap();
        assert!(!head.is_empty());
        assert_eq!(head.len(), 40); // full SHA
    }

    #[test]
    fn detects_changed_files() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        init_git_repo(root);
        std::fs::write(root.join("a.txt"), "a").unwrap();
        git_commit_all(root, "first");

        let first = get_head_commit(root).unwrap();

        std::fs::write(root.join("b.txt"), "b").unwrap();
        std::fs::write(root.join("a.txt"), "modified").unwrap();
        git_commit_all(root, "second");

        let changes = get_changed_files(root, &first).unwrap();
        let paths: Vec<&str> = changes.iter().map(|c| c.path.as_str()).collect();
        assert!(paths.contains(&"b.txt"));
        assert!(paths.contains(&"a.txt"));
    }
}
