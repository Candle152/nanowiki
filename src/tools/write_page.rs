//! WriteWikiPage tool — create or overwrite Markdown documents in .nanowiki/

use std::path::Path;

pub struct WriteResult {
    pub created: bool,
}

/// Create or overwrite a Markdown document under .nanowiki/.
///
/// Auto-creates parent directories. Rejects INDEX.md and .last-update.json.
pub fn write_wiki_page(
    wiki_root: &Path,
    page_name: &str,
    content: &str,
    _summary: &str,
) -> Result<WriteResult, String> {
    crate::tools::validate_relative_path(page_name)?;
    let path = Path::new(page_name);

    if path.extension().and_then(|e| e.to_str()) != Some("md") {
        return Err("only .md files can be written".into());
    }

    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if name == "INDEX.md" || name == ".last-update.json" {
        return Err(format!("writing system-managed file is forbidden: {}", name));
    }

    let full_path = wiki_root.join(path);
    let created = !full_path.exists();

    if let Some(parent) = full_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("cannot create directory: {}", e))?;
    }

    std::fs::write(&full_path, content)
        .map_err(|e| format!("write failed: {}", e))?;

    Ok(WriteResult {
        created,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn setup_wiki() -> (tempfile::TempDir, std::path::PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let wiki = tmp.path().join(".nanowiki");
        fs::create_dir_all(&wiki).unwrap();
        (tmp, wiki)
    }

    #[test]
    fn creates_new_file() {
        let (_tmp, wiki) = setup_wiki();
        let result = write_wiki_page(&wiki, "quickstart.md", "# Hello", "project entry").unwrap();
        assert!(result.created);

        let content = fs::read_to_string(wiki.join("quickstart.md")).unwrap();
        assert_eq!(content, "# Hello");
    }

    #[test]
    fn overwrites_existing_file() {
        let (_tmp, wiki) = setup_wiki();
        fs::write(wiki.join("arch.md"), "old").unwrap();

        let result = write_wiki_page(&wiki, "arch.md", "new", "architecture docs").unwrap();
        assert!(!result.created);

        let content = fs::read_to_string(wiki.join("arch.md")).unwrap();
        assert_eq!(content, "new");
    }

    #[test]
    fn creates_parent_directories() {
        let (_tmp, wiki) = setup_wiki();
        write_wiki_page(&wiki, "architecture/overview.md", "# Arch", "architecture overview").unwrap();

        let content = fs::read_to_string(wiki.join("architecture/overview.md")).unwrap();
        assert_eq!(content, "# Arch");
    }

    #[test]
    fn rejects_index_md() {
        let (_tmp, wiki) = setup_wiki();
        let result = write_wiki_page(&wiki, "INDEX.md", "# idx", "index");
        assert!(result.is_err(), "writing INDEX.md should be rejected");
    }

    #[test]
    fn rejects_last_update_json() {
        let (_tmp, wiki) = setup_wiki();
        let result = write_wiki_page(&wiki, ".last-update.json", "{}", "metadata");
        assert!(result.is_err(), "writing .last-update.json should be rejected");
    }

    #[test]
    fn rejects_non_md_extension() {
        let (_tmp, wiki) = setup_wiki();
        let result = write_wiki_page(&wiki, "notes.txt", "text", "notes");
        assert!(result.is_err(), "non-.md files should be rejected");
    }

    #[test]
    fn rejects_parent_dir_path() {
        let (_tmp, wiki) = setup_wiki();
        let result = write_wiki_page(&wiki, "../outside.md", "# bad", "escape");
        assert!(result.is_err(), ".. path should be rejected");
    }
}
