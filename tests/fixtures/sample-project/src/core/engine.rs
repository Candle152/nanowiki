//! 引擎模块 — 核心业务逻辑

use crate::core::config::AppConfig;
use std::collections::HashMap;
use std::path::PathBuf;

/// 从远程 API 拉取数据
pub async fn fetch_data(cfg: &AppConfig, url: &str) -> anyhow::Result<String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(cfg.timeout_secs))
        .build()?;
    let resp = client.get(url).send().await?;
    let body = resp.text().await?;
    Ok(body)
}

/// 管理本地缓存
pub fn manage_cache(cfg: &AppConfig, action: &str) -> anyhow::Result<()> {
    match action {
        "list" => list_cache(&cfg.cache_dir),
        "clear" => clear_cache(&cfg.cache_dir),
        "prune" => prune_cache(&cfg.cache_dir),
        _ => anyhow::bail!("未知操作: {}", action),
    }
}

fn list_cache(cache_dir: &PathBuf) -> anyhow::Result<()> {
    if !cache_dir.exists() {
        println!("缓存目录为空");
        return Ok(());
    }
    for entry in std::fs::read_dir(cache_dir)? {
        let entry = entry?;
        println!("  {}", entry.file_name().to_string_lossy());
    }
    Ok(())
}

fn clear_cache(cache_dir: &PathBuf) -> anyhow::Result<()> {
    if cache_dir.exists() {
        std::fs::remove_dir_all(cache_dir)?;
    }
    std::fs::create_dir_all(cache_dir)?;
    println!("缓存已清空");
    Ok(())
}

fn prune_cache(cache_dir: &PathBuf) -> anyhow::Result<()> {
    // 清理超过 7 天的缓存条目
    let now = std::time::SystemTime::now();
    if !cache_dir.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(cache_dir)? {
        let entry = entry?;
        if let Ok(meta) = entry.metadata() {
            if let Ok(modified) = meta.modified() {
                if now.duration_since(modified).unwrap_or_default().as_secs() > 7 * 24 * 3600 {
                    if meta.is_dir() {
                        let _ = std::fs::remove_dir_all(entry.path());
                    } else {
                        let _ = std::fs::remove_file(entry.path());
                    }
                }
            }
        }
    }
    println!("过期缓存已清理");
    Ok(())
}

/// 打印统计信息
pub fn print_stats(cfg: &AppConfig) -> anyhow::Result<()> {
    let cache_count = if cfg.cache_dir.exists() {
        std::fs::read_dir(&cfg.cache_dir)?.count()
    } else {
        0
    };
    println!("API 地址: {}", cfg.api_base_url);
    println!("超时: {}s", cfg.timeout_secs);
    println!("重试次数: {}", cfg.retry.max_attempts);
    println!("缓存条目: {}", cache_count);
    Ok(())
}
