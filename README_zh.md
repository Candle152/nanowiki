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

## 为什么用 NanoWiki？

NanoWiki 受 LangChain 团队的 [OpenWiki](https://github.com/langchain-ai/openwiki) 启发，去掉了不需要的功能，并用 Rust 重写为纯命令行工具——不依赖 Node.js 或 Bun 运行时。它的初衷是为 coding agent 服务：生成一份 wiki，让 agent 能快速了解项目。

### 适用场景

- **小型工具项目**：通常不需要生成 wiki，agent 直接读文件就能理解并修改。
- **中大型项目**：结构化文档有轻微优势——修改模块接口时不会遗漏调用方的改动；一次跑通总比发现问题再补更省 token。

### 对信息压缩的尝试

大模型有上下文窗口限制，不可能读完所有文档再写；上下文爆炸会导致信息丢失和 token 开销激增。相比“接近窗口上限就压缩”的策略，NanoWiki 尝试用更少的 token 完成写 wiki 并保证质量——这是本项目的一个核心探索方向，尚未充分验证。

### 现状与边界

- 目前只针对 DeepSeek 做了 thinking 关闭的优化；其他模型在生成 map 时，输出可能全部被 thinking 占用而返回空内容。
- 个人 wiki 暂不涉猎。一个值得思考的问题：你真的会去读 AI 写的文档，而不是直接询问 agent 吗？

### 实测花费

对 OpenWiki 项目生成 wiki（使用 deepseek-v4-flash）：约 18.27M tokens，合计 ¥1.84（$0.27），文档质量可接受。

## 工作原理

`nanowiki init` 分为四个阶段：

1. **扫描** — 遍历仓库（遵循 `.gitignore`）列出所有文件。
2. **提取** — 用 LLM 为每个源文件生成摘要，存入 `.nanowiki/_scan/`。
3. **聚合** — 将全部摘要与静态文件树合并为一份压缩的 `.nanowiki/_map.md`。
4. **撰写** — agent 读取 `_map.md` 起草页面骨架，再读取实际源码逐页填充，最后生成 `quickstart.md`。

agent 使用 8 个工具（列目录、glob 匹配、搜索、读文件、写页面、编辑页面、骨架管理、上下文清理）。

`nanowiki update` 计算自上次运行以来的 `git diff`，只编辑受影响的页面。

仓库根目录生成的 `AGENTS.md` 使任意 AI 编程助手能够自动导航文档。

## 输出结构

```
.nanowiki/
├── INDEX.md              # 自动生成的页面索引
├── quickstart.md          # AI 助手入口页
├── .last-update.json      # 运行元数据（git HEAD、模型、状态）
├── _scan/                 # 每文件摘要（提取阶段）
└── <主题>.md              # 结构化文档（overview、cli、config 等）
```

## 致谢

NanoWiki 受 LangChain 团队的 [OpenWiki](https://github.com/langchain-ai/openwiki) 启发，prompt 工程和输出结构参考了 OpenWiki 的设计。

## 许可证

[MIT](LICENSE)
