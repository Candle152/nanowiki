use std::path::{Path, PathBuf};


/// List directory contents (actual LLM tool implementation).
///
/// Filters out `.git` directories and `.gitignore`-excluded files.
/// Sorted: directories first, then files, each alphabetically. Max 200 entries.
pub fn list_directory(repo_root: &Path, rel_path: &str) -> Result<Vec<DirEntry>, String> {
    let mut entries = list_dir_raw(repo_root, rel_path)?;

    entries.retain(|e| !(e.entry_type == EntryType::Dir && e.name == ".git"));

    entries = apply_gitignore_filter(repo_root, rel_path, entries);

    if entries.len() > 200 {
        entries.truncate(200);
    }

    Ok(entries)
}

#[derive(Debug, PartialEq)]
pub struct DirEntry {
    pub name: String,
    pub entry_type: EntryType,
    pub size: u64,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum EntryType {
    Dir,
    File,
}


fn list_dir_raw(repo_root: &Path, rel_path: &str) -> Result<Vec<DirEntry>, String> {
    let full_path = resolve_path(repo_root, rel_path)?;

    if !full_path.is_dir() {
        return Err(format!("not a directory: {}", rel_path));
    }

    let mut entries: Vec<DirEntry> = Vec::new();
    let read_dir =
        std::fs::read_dir(&full_path).map_err(|e| format!("cannot read directory {}: {}", rel_path, e))?;

    for entry in read_dir {
        let entry = entry.map_err(|e| format!("failed to read entry: {}", e))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let file_type = entry
            .file_type()
            .map_err(|e| format!("cannot get file type: {}", e))?;
        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);

        entries.push(DirEntry {
            name,
            entry_type: if file_type.is_dir() {
                EntryType::Dir
            } else {
                EntryType::File
            },
            size,
        });
    }

    entries.sort_by(|a, b| match (&a.entry_type, &b.entry_type) {
        (EntryType::Dir, EntryType::File) => std::cmp::Ordering::Less,
        (EntryType::File, EntryType::Dir) => std::cmp::Ordering::Greater,
        _ => a.name.cmp(&b.name),
    });

    Ok(entries)
}

fn resolve_path(repo_root: &Path, rel_path: &str) -> Result<PathBuf, String> {
    crate::tools::validate_relative_path(rel_path)?;
    Ok(repo_root.join(Path::new(rel_path)))
}

fn apply_gitignore_filter(
    repo_root: &Path,
    rel_path: &str,
    entries: Vec<DirEntry>,
) -> Vec<DirEntry> {
    let target_dir = repo_root.join(rel_path);
    let mut builder = ignore::WalkBuilder::new(&target_dir);
    builder.require_git(false).max_depth(Some(1));

    let walker = builder.build().filter_map(|r| r.ok());
    let mut visible: std::collections::HashSet<String> = std::collections::HashSet::new();
    for entry in walker {
        if entry.depth() == 1
            && let Some(name) = entry.file_name().to_str() {
                visible.insert(name.to_string());
            }
    }

    if visible.is_empty() {
        return entries;
    }

    entries
        .into_iter()
        .filter(|e| visible.contains(&e.name))
        .collect()
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn setup_temp_dir() -> (tempfile::TempDir, PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().to_path_buf();

        fs::create_dir_all(root.join("src")).unwrap();
        fs::create_dir_all(root.join("tests")).unwrap();
        fs::write(root.join("README.md"), "# Test").unwrap();
        fs::write(root.join("Cargo.toml"), "[package]").unwrap();
        fs::write(root.join("src/main.rs"), "fn main() {}").unwrap();
        fs::write(root.join("src/lib.rs"), "pub fn foo() {}").unwrap();
        fs::write(root.join("tests/test.rs"), "// test").unwrap();

        (tmp, root)
    }

    #[test]
    fn lists_root_directory() {
        let (_tmp, root) = setup_temp_dir();
        let entries = list_dir_raw(&root, "").unwrap();

        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"src"));
        assert!(names.contains(&"tests"));
        assert!(names.contains(&"README.md"));
        assert!(names.contains(&"Cargo.toml"));

        let src_pos = names.iter().position(|&n| n == "src").unwrap();
        let readme_pos = names.iter().position(|&n| n == "README.md").unwrap();
        assert!(src_pos < readme_pos, "directories should come before files");
    }

    #[test]
    fn lists_subdirectory() {
        let (_tmp, root) = setup_temp_dir();
        let entries = list_dir_raw(&root, "src").unwrap();
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["lib.rs", "main.rs"]);
    }

    #[test]
    fn rejects_parent_dir_path() {
        let (_tmp, root) = setup_temp_dir();
        let result = list_dir_raw(&root, "../etc");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains(".."));
    }

    #[test]
    fn rejects_absolute_path() {
        let (_tmp, root) = setup_temp_dir();
        let abs_path = if cfg!(windows) { "C:\\etc" } else { "/etc" };
        let result = list_dir_raw(&root, abs_path);
        assert!(result.is_err(), "should reject absolute path: {}", abs_path);
    }

    #[test]
    fn rejects_non_directory() {
        let (_tmp, root) = setup_temp_dir();
        let result = list_dir_raw(&root, "README.md");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not a directory"));
    }


    #[test]
    fn filters_dot_git_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::create_dir_all(root.join("src_dir")).unwrap();

        let entries = list_directory(root, "").unwrap();
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(!names.contains(&".git"));
        assert!(names.contains(&"src_dir"));
    }

    #[test]
    fn respects_gitignore() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::write(root.join(".gitignore"), "*.log\nbuild/\n").unwrap();
        fs::write(root.join("main.rs"), "").unwrap();
        fs::write(root.join("debug.log"), "").unwrap();
        fs::create_dir_all(root.join("build")).unwrap();
        fs::write(root.join("build/output.o"), "").unwrap();

        let entries = list_directory(root, "").unwrap();
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"main.rs"));
        assert!(!names.contains(&"debug.log"), "*.log should be filtered");
        assert!(!names.contains(&"build"), "build/ dir should be filtered");
    }

    #[test]
    fn respects_gitignore_in_subdirectory() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::write(root.join(".gitignore"), "*.log\n").unwrap();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/main.rs"), "").unwrap();
        fs::write(root.join("src/debug.log"), "").unwrap();

        let entries = list_directory(root, "src").unwrap();
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"main.rs"), "main.rs should be in list");
        assert!(!names.contains(&"debug.log"), "*.log should be filtered in subdirectory");
    }

    #[test]
    fn truncates_at_200_entries() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        for i in 0..210 {
            fs::write(root.join(format!("file_{:04}.txt", i)), "").unwrap();
        }

        let entries = list_directory(root, "").unwrap();
        assert_eq!(entries.len(), 200, "should truncate to 200 entries");
    }
}
