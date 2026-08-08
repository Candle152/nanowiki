//! 入口 — 解析 CLI 命令并路由到对应处理器

mod cli;
mod core;
mod utils;

use clap::Parser;
use cli::Cli;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let cfg = core::config::load()?;

    match cli.command {
        cli::Command::Fetch { url, format } => {
            println!("正在从 {} 拉取数据（格式: {}）...", url, format);
            let data = core::engine::fetch_data(&cfg, &url).await?;
            let output = utils::format::format_output(&data, &format)?;
            println!("{}", output);
        }
        cli::Command::Cache { action } => {
            core::engine::manage_cache(&cfg, &action)?;
        }
        cli::Command::Stats => {
            core::engine::print_stats(&cfg)?;
        }
    }

    Ok(())
}
