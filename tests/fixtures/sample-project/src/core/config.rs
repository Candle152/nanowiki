//! 配置模块 — 从 config/default.toml 加载配置

use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
pub struct AppConfig {
    pub api_base_url: String,
    pub timeout_secs: u64,
    pub cache_dir: PathBuf,
    pub retry: RetryConfig,
}

#[derive(Debug, Deserialize)]
pub struct RetryConfig {
    pub max_attempts: u32,
    pub backoff_ms: u64,
}

/// 从默认路径加载配置
pub fn load() -> anyhow::Result<AppConfig> {
    let content = std::fs::read_to_string("config/default.toml")?;
    let cfg: AppConfig = toml::from_str(&content)?;
    Ok(cfg)
}
