# Sample Project

一个示例 CLI 工具项目，用于验证 NanoWiki 的代码分析能力。

## 功能

- 从远程 API 拉取数据
- 本地缓存，支持多种格式（JSON、TOML）
- 命令行子命令：fetch / cache / stats

## 架构

```
cli/     → 命令行解析与路由
core/    → 核心业务逻辑（引擎 + 配置）
utils/   → 工具函数（格式化、校验）
```
