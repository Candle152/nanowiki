//! LLM tool implementations

pub mod edit_page;
pub mod glob;
pub mod list_dir;
pub mod prune_context;
pub mod read_file;
pub mod search;
pub mod skeleton;
pub mod write_page;


/// Validate a relative path: no `..`, no absolute, no root prefix.
/// Used by all tools that accept user-provided paths.
pub fn validate_relative_path(path: &str) -> Result<(), String> {
    let p = std::path::Path::new(path);
    if p.is_absolute() {
        return Err("absolute paths are not allowed".into());
    }
    if p.has_root() {
        return Err("root paths are not allowed".into());
    }
    if p.components().any(|c| c == std::path::Component::ParentDir) {
        return Err("parent directory traversal (..) is not allowed".into());
    }
    Ok(())
}

/// Truncate a byte slice to `max` bytes at a valid UTF-8 character boundary.
pub fn truncate_utf8(bytes: &[u8], max: usize) -> &[u8] {
    if bytes.len() <= max {
        return bytes;
    }
    let mut end = max;
    while end > 0 && (bytes[end] & 0xC0) == 0x80 {
        end -= 1;
    }
    &bytes[..end]
}
