//! SkeletonTool — structured skeleton management

use std::path::Path;

#[derive(Debug, PartialEq)]
pub struct SkeletonEntry {
    pub page: String,
    pub files: Vec<String>,
    pub done: bool,
}

/// Parse _skeleton.md content into structured entries.
fn parse_skeleton(content: &str) -> Vec<SkeletonEntry> {
    let mut entries = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("- [") {
            continue;
        }
        let done = trimmed.starts_with("- [x]") || trimmed.starts_with("- [x ]");
        // Extract page name and files: "- [x] page.md → file1.rs file2.rs"
        let rest = if done {
            trimmed.strip_prefix("- [x]").or_else(|| trimmed.strip_prefix("- [x ]"))
        } else {
            trimmed.strip_prefix("- [ ]").or_else(|| trimmed.strip_prefix("- []"))
        };
        let rest = match rest {
            Some(r) => r.trim(),
            None => continue,
        };
        // Split on → to get page and files
        let parts: Vec<&str> = rest.splitn(2, '→').collect();
        let page = parts[0].trim().to_string();
        let files: Vec<String> = if parts.len() > 1 {
            parts[1]
                .split_whitespace()
                .map(|s| s.to_string())
                .collect()
        } else {
            vec![]
        };
        entries.push(SkeletonEntry { page, files, done });
    }
    entries
}

/// Get structured skeleton status.
pub fn skeleton_status(wiki_root: &Path, page_name: &str) -> Result<String, String> {
    crate::tools::validate_relative_path(page_name)?;
    let full_path = wiki_root.join(Path::new(page_name));
    if !full_path.is_file() {
        return Err(format!("skeleton file not found: {}", page_name));
    }
    let content = std::fs::read_to_string(&full_path)
        .map_err(|e| format!("cannot read skeleton: {}", e))?;

    let entries = parse_skeleton(&content);
    let mut done = vec![];
    let mut pending = vec![];

    for e in &entries {
        if e.done {
            done.push(format!("  [x] {} → {}", e.page, e.files.join(" ")));
        } else {
            pending.push(format!("  [ ] {} → {}", e.page, e.files.join(" ")));
        }
    }

    let mut out = String::new();
    if !pending.is_empty() {
        out.push_str("## Pending\n");
        for p in &pending {
            out.push_str(p);
            out.push('\n');
        }
    }
    if !done.is_empty() {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str("## Done\n");
        for d in &done {
            out.push_str(d);
            out.push('\n');
        }
    }
    if out.is_empty() {
        out.push_str("(empty skeleton)\n");
    }
    Ok(out.trim().to_string())
}

/// Mark a page as complete in the skeleton.
///
/// `skeleton_page` — path to the skeleton file (e.g. "_skeleton.md").
/// `target_page` — the page name to mark as done (e.g. "agent.md").
pub fn skeleton_complete(
    wiki_root: &Path,
    skeleton_page: &str,
    target_page: &str,
) -> Result<String, String> {
    crate::tools::validate_relative_path(skeleton_page)?;
    let full_path = wiki_root.join(Path::new(skeleton_page));
    if !full_path.is_file() {
        return Err(format!("skeleton file not found: {}", skeleton_page));
    }
    let content = std::fs::read_to_string(&full_path)
        .map_err(|e| format!("cannot read skeleton: {}", e))?;

    // Find the line containing "[ ] target_page" and replace with "[x]"
    let mut found = false;
    let new_content: String = content
        .lines()
        .map(|line| {
            if !found && line.contains(&format!("[ ] {}", target_page)) {
                found = true;
                line.replacen("[ ]", "[x]", 1)
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    // Preserve trailing newline if original had one
    let new_content = if content.ends_with('\n') {
        format!("{}\n", new_content)
    } else {
        new_content
    };

    if !found {
        return Err(format!(
            "page '{}' not found in skeleton (must match skeleton entry exactly)",
            target_page
        ));
    }

    std::fs::write(&full_path, &new_content)
        .map_err(|e| format!("write failed: {}", e))?;

    Ok(format!("marked {} as complete in {}", target_page, skeleton_page))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_pending_entries() {
        let content = "- [ ] overview.md → Cargo.toml README.md\n- [ ] agent.md → src/agent.rs\n";
        let entries = parse_skeleton(content);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].page, "overview.md");
        assert_eq!(entries[0].files, vec!["Cargo.toml", "README.md"]);
        assert!(!entries[0].done);
        assert_eq!(entries[1].page, "agent.md");
        assert_eq!(entries[1].files, vec!["src/agent.rs"]);
        assert!(!entries[1].done);
    }

    #[test]
    fn parses_done_entries() {
        let content = "- [x] agent.md → src/agent.rs\n- [ ] tools.md → src/tools/mod.rs\n";
        let entries = parse_skeleton(content);
        assert!(entries[0].done);
        assert!(!entries[1].done);
    }

    #[test]
    fn parses_mixed_entries() {
        let content = "- [x] done.md → file.rs\n- [ ] pending.md → other.rs\n- [x] another.md\n";
        let entries = parse_skeleton(content);
        assert_eq!(entries.len(), 3);
        assert!(entries[0].done);
        assert!(!entries[1].done);
        assert!(entries[2].done);
        assert!(entries[2].files.is_empty());
    }

    #[test]
    fn ignores_non_checkbox_lines() {
        let content = "# Skeleton\n\n- [ ] page.md → src/file.rs\n\nSome text\n- [x] done.md\n";
        let entries = parse_skeleton(content);
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn parses_various_arrow_formats() {
        let content = "- [ ] tools.md → src/tools/mod.rs src/tools/*.rs\n- [ ] cli.md→src/cli.rs\n";
        let entries = parse_skeleton(content);
        assert_eq!(entries[0].files.len(), 2);
        assert_eq!(entries[1].files.len(), 1);
    }

    // ── skeleton_complete tests ──

    fn setup_wiki() -> (tempfile::TempDir, std::path::PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let wiki = tmp.path().join(".nanowiki");
        std::fs::create_dir_all(&wiki).unwrap();
        (tmp, wiki)
    }

    #[test]
    fn marks_page_as_complete() {
        let (_tmp, wiki) = setup_wiki();
        let skeleton = "- [ ] overview.md → Cargo.toml\n- [ ] agent.md → src/agent.rs\n";
        std::fs::write(wiki.join("_skeleton.md"), skeleton).unwrap();

        let result = skeleton_complete(&wiki, "_skeleton.md", "agent.md").unwrap();
        assert!(result.contains("marked agent.md"));

        let updated = std::fs::read_to_string(wiki.join("_skeleton.md")).unwrap();
        assert!(updated.contains("- [x] agent.md"));
        assert!(updated.contains("- [ ] overview.md"));
    }

    #[test]
    fn complete_preserves_trailing_newline() {
        let (_tmp, wiki) = setup_wiki();
        let skeleton = "- [ ] page.md → file.rs\n";
        std::fs::write(wiki.join("_skeleton.md"), skeleton).unwrap();

        skeleton_complete(&wiki, "_skeleton.md", "page.md").unwrap();
        let updated = std::fs::read_to_string(wiki.join("_skeleton.md")).unwrap();
        assert!(updated.ends_with('\n'));
    }

    #[test]
    fn complete_errors_on_unknown_page() {
        let (_tmp, wiki) = setup_wiki();
        std::fs::write(wiki.join("_skeleton.md"), "- [ ] known.md → file.rs\n").unwrap();

        let result = skeleton_complete(&wiki, "_skeleton.md", "unknown.md");
        assert!(result.is_err());
    }

    #[test]
    fn complete_only_marks_first_match() {
        let (_tmp, wiki) = setup_wiki();
        let skeleton = "- [ ] dup.md → a.rs\n- [ ] dup.md → b.rs\n";
        std::fs::write(wiki.join("_skeleton.md"), skeleton).unwrap();

        skeleton_complete(&wiki, "_skeleton.md", "dup.md").unwrap();
        let updated = std::fs::read_to_string(wiki.join("_skeleton.md")).unwrap();
        let x_count = updated.matches("[x] dup.md").count();
        let open_count = updated.matches("[ ] dup.md").count();
        assert_eq!(x_count, 1);
        assert_eq!(open_count, 1);
    }
}
