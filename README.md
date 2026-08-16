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

## Why NanoWiki?

Inspired by [OpenWiki](https://github.com/langchain-ai/openwiki) by the LangChain team, NanoWiki strips away the parts it doesn't need and runs as a pure Rust CLI — no Node.js or Bun runtime required. It is built for coding agents: generate a wiki so an agent can grasp a project quickly.

### When to use it

- **Small utility projects**: usually unnecessary — an agent can read the files directly and make changes.
- **Medium-to-large projects**: structured docs offer a slight edge — when you change a module's interface you are less likely to miss its callers; getting it right in one pass costs fewer tokens than discovering the problem later.

### A bet on compression

Models have a limited context window — you cannot read every document before writing, and overflowing context means lost information and ballooning token costs. Rather than "compress when the window is nearly full", NanoWiki tries to write the wiki with fewer tokens while keeping quality. This is one of the project's core explorations, and it has not been fully verified yet.

### Current scope

- Only DeepSeek's thinking mode is handled today. With other models, the map step may return nothing because the output is consumed entirely by thinking tokens.
- Personal wikis are out of scope for now. A question worth asking: would you actually read AI-written docs instead of just asking the agent?

### Cost

Generating a wiki for the OpenWiki project (deepseek-v4-flash): ~18.27M tokens, about $0.27 (¥1.84), with acceptable quality.

## How It Works

`nanowiki init` runs a four-phase pipeline:

1. **Scan** — walk the repository (respecting `.gitignore`) to list all files.
2. **Extract** — summarize each source file with the LLM into `.nanowiki/_scan/`.
3. **Map** — aggregate the summaries plus a static file tree into one compressed `.nanowiki/_map.md`.
4. **Write** — the agent reads `_map.md`, drafts a page skeleton, then reads the actual source files to fill in each page, and finally writes `quickstart.md`.

The agent uses 8 tools (list directory, glob, search, read file, write page, edit page, skeleton, prune context).

`nanowiki update` computes `git diff` since the last run and edits only the affected pages.

The generated `AGENTS.md` at the repo root enables any AI coding agent to navigate the docs automatically.

## Output Structure

```
.nanowiki/
├── INDEX.md              # Auto-generated page index
├── quickstart.md          # Entry point for AI agents
├── .last-update.json      # Run metadata (git HEAD, model, status)
├── _scan/                 # Per-file summaries (extract phase)
└── <topic>.md             # Structured docs (overview, cli, config, ...)
```

## Acknowledgments

NanoWiki is inspired by [OpenWiki](https://github.com/langchain-ai/openwiki) by the LangChain team. The prompt engineering and output structure draw from OpenWiki's design.

## License

[MIT](LICENSE)
