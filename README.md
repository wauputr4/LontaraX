# LontaraX

**LontaraX** is a lightweight AI API Router CLI inspired by *Lontara*—the Bugis-Makassar tradition of writing, recording, and preserving knowledge.

The **X** stands for:
- **Cross-provider**
- **Extensibility**
- **Exchange layer**

## What it is

LontaraX gives developers and AI agents a single CLI interface to access multiple AI providers through a consistent abstraction.

Instead of learning every provider’s quirks, you interact with one router:
- `lontarax credits`
- `lontarax models --provider kie`
- `lontarax chat "hello"`
- `lontarax image "cat astronaut"`
- `lontarax history`

## Why SQLite

SQLite keeps LontaraX:
- lightweight
- single-binary friendly
- easy to ship
- local-first
- reliable for caching provider models, request history, and task metadata

## Why modular providers

Each provider lives in its own crate so new integrations can be developed independently.
This keeps the core small and makes provider-specific code easier to maintain.

## Install

```bash
cargo build --release -p lontarax
```

Binary output:
- Linux/macOS: `target/release/lontarax`
- Windows: `target/release/lontarax.exe`

## Configuration

Default config path:

```text
~/.config/lontarax/config.yaml
```

Default database path:

```text
~/.local/share/lontarax/lontarax.db
```

Environment overrides:
- `LONTARAX_KIE_API_KEY`
- `LONTARAX_DEFAULT_PROVIDER`
- `LONTARAX_DATABASE_URL`
- `LONTARAX_CONFIG_PATH`

## Quick start

```bash
lontarax init
lontarax provider add kie --api-key sk-xxxx
lontarax provider list
lontarax credits --provider kie
lontarax models --provider kie
```

## Adding a new provider

1. Create a new crate under `crates/providers/`
2. Implement the `AiProvider` trait
3. Implement a `ProviderFactory`
4. Register the factory in the bootstrap layer
5. Add tests using a mock HTTP server

## Current roadmap

### v0.1
- Kie AI provider
- SQLite local state
- provider registry
- model registry
- credits
- history
- basic CLI commands

### v0.2
- async task polling
- download generated files
- shell completion
- output formatter

### v0.3
- OpenRouter provider
- OpenAI-compatible provider
- fallback routing
- usage tracking
- cost tracking

### v0.4
- Ollama provider
- MCP bridge
- policy-based routing
- auto model selection

## Notes

- Output defaults to human-readable, with `--json` for machine-readable output.
- Secrets are masked in provider listings.
- Kie image workflow is wired via task-based create/poll flow; chat/video remain scaffolded.
