//! CLI 命令处理器

use crate::core::engine;

/// 执行 fetch 命令的预处理
pub fn validate_url(url: &str) -> Result<(), String> {
    if url.starts_with("http://") || url.starts_with("https://") {
        Ok(())
    } else {
        Err(format!("无效的 URL: {}", url))
    }
}

/// 执行 cache 命令的预处理
pub fn validate_cache_action(action: &str) -> Result<(), String> {
    match action {
        "list" | "clear" | "prune" => Ok(()),
        other => Err(format!("未知的缓存操作: {}", other)),
    }
}
