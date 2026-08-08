//! CLI 模块 — 命令定义与解析

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "sample", about = "示例数据拉取工具")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// 从远程 URL 拉取数据
    Fetch {
        /// 目标 URL
        url: String,
        /// 输出格式 (json / toml / yaml)
        #[arg(short, long, default_value = "json")]
        format: String,
    },
    /// 管理本地缓存
    Cache {
        /// 操作: list / clear / prune
        action: String,
    },
    /// 显示统计信息
    Stats,
}
