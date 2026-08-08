//! EditWikiPage tool — precise text replacement within .nanowiki/ documents

use std::path::Path;

/// Replace old_text with new_text in a .nanowiki/ wiki page.
///
/// old_text must match exactly once in the file.
pub fn edit_wiki_page(
    wiki_root: &Path,
    page_name: &str,
    old_text: &str,
    new_text: &str,
) -> Result<(), String> {
    crate::tools::validate_relative_path(page_name)?;
    let path = Path::new(page_name);

    if path.extension().and_then(|e| e.to_str()) != Some("md") {
        return Err("only .md files can be edited".into());
    }

    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if name == "INDEX.md" || name == ".last-update.json" {
        return Err(format!("editing system-managed file is forbidden: {}", name));
    }

    let full_path = wiki_root.join(path);

    if !full_path.is_file() {
        return Err(format!("file not found: {}", page_name));
    }

    let content = std::fs::read_to_string(&full_path)
        .map_err(|e| format!("cannot read file: {}", e))?;

    let count = content.matches(old_text).count();
    if count == 0 {
        return Err("no match found for the provided text".into());
    }
    if count > 1 {
        return Err(format!("{} matches found, provide more context to uniquely identify the replacement", count));
    }

    let new_content = content.replacen(old_text, new_text, 1);
    std::fs::write(&full_path, new_content)
        .map_err(|e| format!("write failed: {}", e))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn setup_wiki_with_file(name: &str, content: &str) -> (tempfile::TempDir, std::path::PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let wiki = tmp.path().join(".nanowiki");
        fs::create_dir_all(&wiki).unwrap();
        fs::write(wiki.join(name), content).unwrap();
        (tmp, wiki)
    }

    #[test]
    fn replaces_exact_match() {
        let (_tmp, wiki) = setup_wiki_with_file("arch.md", "## Old Title\ncontent");
        edit_wiki_page(&wiki, "arch.md", "## Old Title", "## New Title").unwrap();

        let content = fs::read_to_string(wiki.join("arch.md")).unwrap();
        assert_eq!(content, "## New Title\ncontent");
    }

    #[test]
    fn replaces_only_first_when_unique() {
        let (_tmp, wiki) = setup_wiki_with_file("arch.md", "A B C");
        edit_wiki_page(&wiki, "arch.md", "B", "X").unwrap();

        let content = fs::read_to_string(wiki.join("arch.md")).unwrap();
        assert_eq!(content, "A X C");
    }

    #[test]
    fn error_on_multiple_matches() {
        let (_tmp, wiki) = setup_wiki_with_file("arch.md", "dup dup dup");
        let result = edit_wiki_page(&wiki, "arch.md", "dup", "new");
        assert!(result.is_err(), "multiple matches should error");
    }

    #[test]
    fn error_on_no_match() {
        let (_tmp, wiki) = setup_wiki_with_file("arch.md", "content");
        let result = edit_wiki_page(&wiki, "arch.md", "nonexistent", "new");
        assert!(result.is_err(), "no match should error");
    }

    #[test]
    fn error_on_file_not_found() {
        let tmp = tempfile::tempdir().unwrap();
        let wiki = tmp.path().join(".nanowiki");
        fs::create_dir_all(&wiki).unwrap();

        let result = edit_wiki_page(&wiki, "missing.md", "old", "new");
        assert!(result.is_err(), "missing file should error");
    }

    #[test]
    fn rejects_parent_dir_path() {
        let (_tmp, wiki) = setup_wiki_with_file("arch.md", "content");
        let result = edit_wiki_page(&wiki, "../outside.md", "old", "new");
        assert!(result.is_err(), ".. path should be rejected");
    }

    #[test]
    fn rejects_absolute_path() {
        let (_tmp, wiki) = setup_wiki_with_file("arch.md", "content");
        let abs = if cfg!(windows) { "C:\\outside.md" } else { "/outside.md" };
        let result = edit_wiki_page(&wiki, abs, "old", "new");
        assert!(result.is_err(), "absolute path should be rejected");
    }

    #[test]
    fn rejects_index_md() {
        let (_tmp, wiki) = setup_wiki_with_file("INDEX.md", "content");
        let result = edit_wiki_page(&wiki, "INDEX.md", "old", "new");
        assert!(result.is_err(), "editing INDEX.md should be rejected");
    }

    #[test]
    fn rejects_last_update_json() {
        let (_tmp, wiki) = setup_wiki_with_file(".last-update.json", "{}");
        let result = edit_wiki_page(&wiki, ".last-update.json", "old", "new");
        assert!(result.is_err(), "editing .last-update.json should be rejected");
    }

    #[test]
    fn rejects_non_md_extension() {
        let (_tmp, wiki) = setup_wiki_with_file("notes.txt", "text");
        let result = edit_wiki_page(&wiki, "notes.txt", "old", "new");
        assert!(result.is_err(), "non-.md files should be rejected");
    }
}
