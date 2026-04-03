# RustBot

🐈 Ultra-Lightweight Personal AI Assistant Framework (Rust Implementation)

This is a complete Rust rewrite of [nanobot](https://github.com/HKUDS/nanobot), designed for:

- **Performance**: 10-100x faster than Python
- **Low Memory**: No GC, minimal footprint
- **Single Binary**: Easy deployment, no dependencies
- **Type Safety**: Compile-time guarantees

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

### Phase 4 - Channels

- [ ] Telegram channel
- [ ] Discord channel
- [ ] Feishu channel
- [ ] WhatsApp bridge

### Phase 5 - Services

- [ ] Cron service
- [ ] Heartbeat service
- [ ] OpenAI-compatible API server

### Phase 6 - Advanced

- [ ] MCP (Model Context Protocol)
- [ ] Subagent system
- [ ] Skills loading

## Development

### Prerequisites

- Rust 1.75+ (stable)
- Tokio runtime

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
