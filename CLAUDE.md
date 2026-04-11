# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Test Commands

```bash
# Build (debug)
cargo build

# Build (release)
cargo build --release

# Run all tests
cargo test --workspace

# Run single crate tests
cargo test -p nanobot-core
cargo test -p nanobot-config
cargo test -p nanobot-bus

# Run single test
cargo test -p nanobot-core test_count_tokens

# Lint
cargo clippy --all-targets

# Format
cargo fmt --all

# Run CLI
cargo run --bin rustbot -- agent -m "Hello!"
```

## Binary Commands

```bash
# Initialize configuration
rustbot onboard

# Chat with agent
rustbot agent -m "message"      # Single message
rustbot agent                    # Interactive mode
rustbot agent --logs             # Show logs during chat

# API server (OpenAI-compatible)
rustbot api --port 8900          # No auth
rustbot api --port 8900 --api-key test123  # With auth

# Cron management
rustbot cron add <name> "*/5 * * * * *"
rustbot cron list
rustbot cron remove <name>

# Services management
rustbot services status
rustbot services start <name>
rustbot services stop <name>

# Channel authentication
rustbot channels login telegram
rustbot channels status

# MCP (Model Context Protocol)
rustbot mcp list               # List configured MCP servers
rustbot mcp status             # Show MCP server status
```

## Workspace Architecture

7 crates in workspace:

| Crate | Purpose |
|-------|---------|
| `nanobot-config` | Configuration loading, schema definitions, path management |
| `nanobot-providers` | LLM provider trait, registry, OpenAI-compatible implementations |
| `nanobot-bus` | Async MPSC message bus for inter-component communication |
| `nanobot-core` | Agent loop, tool registry, session management, memory, services, MCP, subagents, skills |
| `nanobot-channels` | Channel connectors (Telegram, Discord, Feishu) |
| `nanobot-api` | OpenAI-compatible HTTP API server (axum) |
| `nanobot-cli` | CLI interface (clap), commands, TUI streaming |

## Core Data Flow

```
Channel (Telegram/Discord/API) 
    → InboundMessage → MessageBus → AgentLoop 
    → LLM Provider ↔ ToolRegistry 
    → OutboundMessage → MessageBus → Channel
```

## Key Design Patterns

**MessageBus**: Cloneable MPSC channels for lock-free message passing between components. `MessageBus` uses `Arc` internally.

**Provider Trait**: `LLMProvider` trait abstracts over different LLM backends. Most providers use `OpenAiCompatProvider`.

**Session Persistence**: Sessions stored as JSON files in `~/.nanobot/sessions/`. `SessionManager` handles CRUD operations.

**Memory Management**: `MemoryManager` handles token counting (tiktoken-rs) and context window truncation.

**Service Architecture**: Cron and Heartbeat services in `nanobot-core/src/services/` - async tasks with RwLock state.

**API Auth Middleware**: Bearer token validation via axum middleware. Multiple keys supported via `ApiAuth`.

**MCP Client**: `nanobot-core/src/mcp/` - Model Context Protocol implementation with stdio/SSE transports.
Fully integrated with AgentLoop - MCP tools automatically registered and available to LLM.

**Subagent System**: `nanobot-core/src/subagent.rs` - Task delegation to specialized agents (Code, Review, Planning, Research, Custom) (Phase 6.2).

**Skills System**: `nanobot-core/src/skills.rs` - Pluggable skill architecture with Memory, CodeReview, Planning built-ins (Phase 6.3).

## Configuration

Config file: `~/.nanobot/config.json`

```json
{
  "providers": {
    "openrouter": { "apiKey": "..." }
  },
  "agents": {
    "defaults": {
      "model": "anthropic/claude-opus-4-5",
      "provider": "openrouter"
    }
  },
  "api": {
    "host": "127.0.0.1",
    "port": 8900
  },
  "channels": {
    "telegram": { "bot_token": "..." }
  },
  "tools": {
    "mcpServers": {
      "filesystem": {
        "transportType": "stdio",
        "command": "npx",
        "args": ["-y", "@modelcontextprotocol/server-filesystem", "~"]
      },
      "sqlite": {
        "transportType": "stdio",
        "command": "npx",
        "args": ["-y", "@modelcontextprotocol/server-sqlite", "/path/to/db.sqlite"]
      },
      "sse-example": {
        "transportType": "sse",
        "url": "http://localhost:3000/sse"
      }
    }
  }
}
```

**MCP Configuration Fields:**

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `transportType` | string | "stdio" | Transport type: "stdio" or "sse" |
| `command` | string | - | Command to spawn (stdio only) |
| `args` | array | [] | Command arguments (stdio only) |
| `env` | object | {} | Environment variables (stdio only) |
| `url` | string | - | SSE endpoint URL (sse only) |
| `headers` | object | {} | HTTP headers (sse only) |
| `toolTimeout` | number | 30 | Tool call timeout in seconds |
| `enabledTools` | array | ["*"] | Tools to enable (wildcard supported) |

Auth tokens stored separately in `~/.nanobot/auth.json` (0600 permissions).

## Common Development Tasks

**Add a provider**: Add `ProviderSpec` to `registry.rs`, add config to `ProvidersConfig`, implement `LLMProvider` if not OpenAI-compatible.

**Add a tool**: Implement `Tool` trait in `crates/nanobot-core/src/tools/`, register in `ToolRegistry`.

**Add a channel**: Implement `ChannelConnector` trait in `crates/nanobot-channels/src/`, add to `ChannelRegistry`.

**Add a service**: Create module in `crates/nanobot-core/src/services/`, use RwLock for state, tokio::spawn for background task.

**Add MCP transport/protocol**: Extend `crates/nanobot-core/src/mcp/`, follow JSON-RPC 2.0 spec 2025-06-18.

**Add a subagent type**: Add `BuiltinSubagent` variant in `subagent.rs`, implement `spec()` and system prompt methods.

**Add a skill**: Implement `Skill` trait in `crates/nanobot-core/src/skills.rs`, register in `SkillRegistry`.

**Add CLI command**: Add to `crates/nanobot-cli/src/commands/`, export in `mod.rs`, wire up in `main.rs`.

## Testing Notes

- Use `anyhow` for CLI/integration test errors
- Use `thiserror` for library error types
- `tempfile` crate for temp config files in tests
- `#[tokio::test]` for async tests

## Phase Status

- ✅ Phase 1: Core infrastructure (config, providers, bus, CLI)
- ✅ Phase 2: Tools (shell, fs, web search, web fetch)
- ✅ Phase 3: Memory & sessions (token counting, truncation, cleanup)
- ✅ Phase 4: Channels (Telegram, Discord, Feishu connectors)
- ✅ Phase 5: Services (Cron, Heartbeat, API server)
- ✅ Phase 6: Advanced (MCP, subagents, skills)

## API Server Endpoints

| Endpoint | Method | Auth | Description |
|----------|--------|------|-------------|
| `/health` | GET | No | Health check |
| `/v1/models` | GET | Yes | List models |
| `/v1/models/:id` | GET | Yes | Get model info |
| `/v1/chat/completions` | POST | Yes | Chat (streaming via SSE) |

## Known Limitations

- Cron jobs in-memory only (no persistence to disk)
- API streaming uses simplified SSE format (full AgentLoop integration pending)
- Subagent execution is placeholder (needs AgentLoop spawn implementation)
- Skills are loaded at runtime (no hot-reload support yet)
- Heartbeat not integrated with session cleanup
- API streaming simplified (AgentLoop integration pending)
- MCP tools require manual server setup (e.g., via npx)
