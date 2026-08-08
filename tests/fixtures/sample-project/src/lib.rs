//! 库入口 — 导出公共 API

pub mod cli;
pub mod core;
pub mod utils;

/// 库版本号
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
