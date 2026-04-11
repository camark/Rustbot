# RustBot

🐈 Ultra-Lightweight Personal AI Assistant Framework (Rust Implementation)

This is a complete Rust rewrite of [nanobot](https://github.com/HKUDS/nanobot), designed for:

- **Performance**: 10-100x faster than Python
- **Low Memory**: No GC, minimal footprint
- **Single Binary**: Easy deployment, no dependencies
- **Type Safety**: Compile-time guarantees

## Languages (语言)

- [English](README.md)
- [简体中文](README.zh-CN.md)
- [日本語](README.ja.md)
- [Deutsch](README.de.md)
- [Français](README.fr.md)

## Project Structure

```
RustBot/
├── Cargo.toml              # Workspace root
├── crates/
│   ├── nanobot-config/     # Configuration system
│   ├── nanobot-providers/  # LLM providers
│   ├── nanobot-bus/        # Message bus
│   ├── nanobot-core/       # Agent engine
│   └── nanobot-cli/        # CLI interface
└── docs/
```

## Quick Start

### Build

```bash
cargo build --release
```

### Initialize

```bash
./target/release/rustbot onboard
```

### Chat

```bash
./target/release/rustbot agent -m "Hello!"
```

### Interactive Mode

```bash
./target/release/rustbot agent
```

### Status

```bash
./target/release/rustbot status
```

## Configuration

Config file: `~/.nanobot/config.json`

```json
{
  "providers": {
    "openrouter": {
      "apiKey": "sk-or-v1-xxx"
    }
  },
  "agents": {
    "defaults": {
      "model": "anthropic/claude-opus-4-5",
      "provider": "openrouter"
    }
  }
}
```

## Supported Providers

| Provider | Status |
|----------|--------|
| OpenRouter | ✅ |
| Anthropic | ✅ |
| OpenAI | ✅ |
| DeepSeek | ✅ |
| Azure OpenAI | ✅ |
| Ollama (local) | ✅ |
| vLLM (local) | ✅ |
| Groq | ✅ |
| Moonshot | ✅ |
| Gemini | ✅ |
| Zhipu | ✅ |
| DashScope | ✅ |

## Roadmap

### Phase 1 - Core Infrastructure ✅

- [x] Config system (compatible with Python nanobot)
- [x] Provider registry and implementations
- [x] Message bus
- [x] CLI framework
- [x] Agent loop skeleton

### Phase 2 - Tools Implementation ✅

- [x] Shell execution tool
- [x] File system tools (read/write/edit/list)
- [x] Web search tool
- [x] Web fetch tool
- [x] All tests passing

### Phase 3 - Memory & Sessions ✅

- [x] Session persistence (JSON file storage)
- [x] Memory consolidation (cleanup expired sessions)
- [x] Context window management (token counting and truncation)
- [x] All tests passing (7/7)

### Phase 4 - Channels ✅

- [x] Channel connector trait
- [x] Telegram channel (teloxide)
- [x] Discord channel (serenity)
- [x] Feishu channel
- [x] Channel registry and manager
- [x] Authentication storage
- [x] CLI commands (login/status)

### Phase 5 - Services ✅

- [x] Cron service
- [x] Heartbeat service
- [x] OpenAI-compatible API server
- [x] Service manager integration

### Phase 6 - Advanced Features ✅

- [x] MCP (Model Context Protocol) client
- [x] Subagent system
- [x] Skills loading system

## Development

### Prerequisites

- Rust 1.75+ (stable)
- Tokio runtime

> **Note for Ubuntu users: Install protoc**
>
> If you encounter protoc-related errors during build, install the Protocol Buffers compiler:
>
> ```bash
> # Ubuntu/Debian
> sudo apt-get update && sudo apt-get install -y protobuf-compiler
>
> # Verify installation
> protoc --version
> ```

### Run Tests

```bash
cargo test
```

### Run Clippy

```bash
cargo clippy --all-targets
```

### Format

```bash
cargo fmt --all
```

## License

MIT License - same as the original nanobot project.

## Acknowledgments

This project is a Rust rewrite inspired by [nanobot](https://github.com/HKUDS/nanobot), an ultra-lightweight Python AI assistant framework.
