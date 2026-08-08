[English](README.md) | **中文**

# NanoWiki

> 用 Rust 编写的轻量级代码知识库生成器。

NanoWiki 使用 LLM 扫描你的代码仓库并生成结构化文档——架构、CLI、核心引擎、运维、集成——全部存放在 `.nanowiki/` 目录中，[pi](https://github.com/anthropics/pi) 等 AI 编程助手可自动发现并使用。

## 安装

```bash
cargo install nanowiki
```

需要 Rust 1.85+（edition 2024）。

## 快速开始

```bash
# 首次运行 — 交互式配置 provider，然后扫描仓库生成文档
nanowiki init

# 代码变更后 — 增量更新受影响的文档
nanowiki update

# 列出已配置的 provider
nanowiki list

# 交互式切换默认 provider/model
nanowiki default
```

## 配置

配置文件位于 `~/.nanowiki/config.json`。首次运行 `init` 会引导你完成配置。

```json
{
    "default_provider": "openai",
    "current_model": "gpt-4o",
    "providers": {
        "openai": {
            "provider_type": "openai",
            "api_key": "sk-...",
            "models": ["gpt-4o", "gpt-4o-mini"]
        },
        "claude": {
            "provider_type": "anthropic",
            "api_key": "sk-ant-...",
            "models": ["claude-sonnet-4-20250514"]
        },
        "ollama": {
            "provider_type": "openai",
            "api_key": "ollama",
            "base_url": "http://localhost:11434/v1",
            "models": ["llama3", "mistral"]
        }
    }
}
```

支持任意 OpenAI 兼容 API（DeepSeek、Ollama、Groq 等）以及 Anthropic 原生 API。

## 工作原理

1. **`nanowiki init`** — 扫描仓库，将文件树交给 LLM，LLM 使用 6 个工具（列目录、glob 匹配、搜索、读文件、写页面、编辑页面）自主探索代码库，在 `.nanowiki/` 下生成结构化 Markdown 文档。
2. **`nanowiki update`** — 计算自上次运行以来的 `git diff`，让 LLM 只编辑受影响的页面。

仓库根目录生成的 `AGENTS.md` 使任意 AI 编程助手能够自动导航文档。

## 输出结构

```
.nanowiki/
├── INDEX.md              # 自动生成的页面索引
├── .last-update.json      # 运行元数据
├── quickstart.md          # AI 助手入口页
├── architecture/          # 架构文档
├── cli/                   # CLI 命令文档
├── agent/                 # 核心引擎文档
├── operations/            # 构建与部署文档
└── integrations/          # 外部依赖文档
```

## 致谢

NanoWiki 受 Anthropic 的 [OpenWiki](https://github.com/anthropics/openwiki) 启发，prompt 工程和输出结构参考了 OpenWiki 的设计。

## 许可证

[MIT](LICENSE)
