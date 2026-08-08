//! Output orchestration — INDEX.md / AGENTS.md / .last-update.json

use serde::{Deserialize, Serialize};
use std::path::Path;


/// Update INDEX.md: insert or update an entry for page_name (with a one-line summary).
pub fn update_index(wiki_root: &Path, page_name: &str, summary: &str) -> Result<(), String> {
    let path = index_path(wiki_root);

    let mut content = if path.exists() {
        std::fs::read_to_string(&path).map_err(|e| format!("cannot read INDEX.md: {}", e))?
    } else {
        String::from("# NanoWiki Index\n\n## Pages\n\n## Directories\n")
    };

    let entry_line = format!("- [{}]({}) - {}", title_from_page(page_name), page_name, summary);
    let link_name = format!("]({})", page_name);

    if content.contains(&link_name) {
        let lines: Vec<&str> = content.lines().collect();
        let new_lines: Vec<String> = lines
            .iter()
            .map(|line| {
                if line.contains(&link_name) {
                    entry_line.clone()
                } else {
                    line.to_string()
                }
            })
            .collect();
        content = new_lines.join("\n");
    } else {
        if let Some(pos) = content.find("## Directories") {
            content.insert_str(pos, &format!("{}\n", entry_line));
        } else {
            content.push_str(&format!("\n{}\n", entry_line));
        }
    }

    if let Some(parent) = Path::new(page_name).parent()
        && parent != Path::new("") {
            let dir_name = parent.to_string_lossy().replace('\\', "/");
            let dir_entry = format!("- [{}]({}/)", dir_name, dir_name);
            if !content.contains(&dir_entry)
                && let Some(pos) = content.find("## Directories") {
                    let insert_pos = content[pos..].find('\n').map(|p| pos + p + 1).unwrap_or(content.len());
                    content.insert_str(insert_pos, &format!("{}\n", dir_entry));
                }
        }

    std::fs::write(&path, content).map_err(|e| format!("cannot write INDEX.md: {}", e))?;
    Ok(())
}

fn title_from_page(page_name: &str) -> String {
    Path::new(page_name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(page_name)
        .replace(['-', '_'], " ")
}

pub fn index_path(wiki_root: &Path) -> std::path::PathBuf {
    wiki_root.join("INDEX.md")
}


#[derive(Debug, Serialize, Deserialize)]
pub struct LastUpdate {
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
    pub command: String,
    #[serde(rename = "gitHead")]
    pub git_head: String,
    pub model: String,
    pub status: String,
}

pub fn write_last_update(
    wiki_root: &Path,
    command: &str,
    git_head: &str,
    model: &str,
    status: &str,
) -> Result<(), String> {
    let lu = LastUpdate {
        updated_at: chrono_now(),
        command: command.to_string(),
        git_head: git_head.to_string(),
        model: model.to_string(),
        status: status.to_string(),
    };
    let json = serde_json::to_string_pretty(&lu).map_err(|e| e.to_string())?;
    std::fs::write(wiki_root.join(".last-update.json"), json).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn read_last_update(wiki_root: &Path) -> Result<Option<LastUpdate>, String> {
    let path = wiki_root.join(".last-update.json");
    if !path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let lu: LastUpdate = serde_json::from_str(&content).map_err(|e| e.to_string())?;
    Ok(Some(lu))
}

fn chrono_now() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn write_and_read_last_update() {
        let tmp = tempfile::tempdir().unwrap();
        let wiki = tmp.path();

        write_last_update(wiki, "init", "abc123", "gpt-4o", "complete").unwrap();

        let lu = read_last_update(wiki).unwrap().unwrap();
        assert_eq!(lu.command, "init");
        assert_eq!(lu.git_head, "abc123");
        assert_eq!(lu.model, "gpt-4o");
        assert_eq!(lu.status, "complete");
    }

    #[test]
    fn read_last_update_returns_none_when_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let result = read_last_update(tmp.path()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn update_index_creates_new_file() {
        let tmp = tempfile::tempdir().unwrap();
        let wiki = tmp.path();

        update_index(wiki, "quickstart.md", "project quickstart").unwrap();

        let content = fs::read_to_string(wiki.join("INDEX.md")).unwrap();
        assert!(content.contains("quickstart.md"));
        assert!(content.contains("project quickstart"));
    }

    #[test]
    fn update_index_adds_multiple_entries() {
        let tmp = tempfile::tempdir().unwrap();
        let wiki = tmp.path();

        update_index(wiki, "quickstart.md", "entry point").unwrap();
        update_index(wiki, "architecture/overview.md", "architecture overview").unwrap();

        let content = fs::read_to_string(wiki.join("INDEX.md")).unwrap();
        assert!(content.contains("quickstart.md"));
        assert!(content.contains("architecture/overview.md"));
    }

    #[test]
    fn update_index_updates_existing_entry() {
        let tmp = tempfile::tempdir().unwrap();
        let wiki = tmp.path();

        update_index(wiki, "arch.md", "old description").unwrap();
        update_index(wiki, "arch.md", "new description").unwrap();

        let content = fs::read_to_string(wiki.join("INDEX.md")).unwrap();
        assert!(!content.contains("old description"));
        assert!(content.contains("new description"));
        assert_eq!(content.matches("arch.md").count(), 1);
    }
}
