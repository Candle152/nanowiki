**English** | [中文](README_zh.md)

# NanoWiki

> A lightweight code knowledge base generator in Rust.

NanoWiki uses an LLM to scan your repository and generate structured documentation — architecture, CLI, agent logic, operations, and integrations — all in a `.nanowiki/` directory that AI coding agents like [pi](https://github.com/anthropics/pi) can automatically discover and use.

## Installation

```bash
cargo install nanowiki
```

Requires Rust 1.85+ (edition 2024).

## Quick Start

```bash
# First run — interactive provider setup, then scan the repo and generate docs
nanowiki init

# After code changes — incrementally update affected docs
nanowiki update

# List configured providers
nanowiki list

# Switch default provider/model interactively
nanowiki default
```

## Configuration

NanoWiki stores its config at `~/.nanowiki/config.json`. The first run (`init`) walks you through setup interactively.

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

Supports any OpenAI-compatible API (DeepSeek, Ollama, Groq, etc.) and Anthropic's native API.

## How It Works

1. **`nanowiki init`** — scans the repo, feeds the file tree to an LLM, and the LLM explores the codebase using 6 tools (list directory, glob, search, read file, write page, edit page). It produces structured Markdown docs under `.nanowiki/`.
2. **`nanowiki update`** — computes `git diff` since the last run and tells the LLM to edit only the affected pages.

The generated `AGENTS.md` at the repo root enables any AI coding agent to navigate the docs automatically.

## Output Structure

```
.nanowiki/
├── INDEX.md              # Auto-generated page index
├── .last-update.json      # Run metadata
├── quickstart.md          # Entry point for AI agents
├── architecture/          # Architecture docs
├── cli/                   # CLI / command docs
├── agent/                 # Core engine docs
├── operations/            # Build & deploy docs
└── integrations/          # External dependency docs
```

## Acknowledgments

NanoWiki is inspired by [OpenWiki](https://github.com/anthropics/openwiki) by Anthropic. The prompt engineering and output structure draw from OpenWiki's design.

## License

[MIT](LICENSE)
