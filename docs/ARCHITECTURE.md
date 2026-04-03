# RustBot Architecture

## Overview

RustBot is a complete Rust rewrite of the Python nanobot framework, designed for performance, safety, and minimal resource usage.

## Core Components

### 1. Configuration System (`nanobot-config`)

**Location**: `crates/nanobot-config/src/`

**Responsibilities**:
- Load/save `config.json` (compatible with Python nanobot format)
- Support both camelCase and snake_case field names
- Environment variable overrides via `NANOBOT_` prefix
- Path management (~/.nanobot/)

**Key Types**:
- `Config` - Root configuration structure
- `ConfigLoader` - Load/save operations
- `ConfigPaths` - Directory paths management

### 2. Provider System (`nanobot-providers`)

**Location**: `crates/nanobot-providers/src/`

**Responsibilities**:
- Unified `LLMProvider` trait interface
- Provider registry for auto-detection
- OpenAI-compatible implementation (works with most providers)
- Streaming support

**Supported Providers**:
- Gateways: OpenRouter, AiHubMix, VolcEngine, BytePlus
- Direct: Anthropic, OpenAI, DeepSeek, Moonshot, Gemini, etc.
- Local: Ollama, vLLM, OVMS

**Key Types**:
- `LLMProvider` (trait) - Provider interface
- `ProviderSpec` - Provider metadata
- `OpenAiCompatProvider` - OpenAI-compatible implementation
- `ChatRequest`, `LLMResponse` - Request/response types

### 3. Message Bus (`nanobot-bus`)

**Location**: `crates/nanobot-bus/src/`

**Responsibilities**:
- Async message queue for inter-component communication
- Inbound/outbound message separation
- Session-based routing

**Key Types**:
- `MessageBus` - Main bus with channels
- `InboundMessage` - Messages from channels
- `OutboundMessage` - Messages to channels

### 4. Core Agent Engine (`nanobot-core`)

**Location**: `crates/nanobot-core/src/`

**Responsibilities**:
- Agent loop (LLM ↔ tool execution)
- Tool registry and execution
- Session management
- Context building
- Hook system

**Key Types**:
- `AgentLoop` - Main processing engine
- `ToolRegistry` - Tool management
- `SessionManager` - Conversation persistence
- `ContextBuilder` - Prompt construction

### 5. CLI (`nanobot-cli`)

**Location**: `crates/nanobot-cli/src/`

**Responsibilities**:
- Command parsing (clap)
- Interactive chat mode
- Gateway server
- Setup wizard

**Commands**:
- `onboard` - Initialize configuration
- `agent` - Chat with the AI
- `gateway` - Start channel server
- `status` - Show system status
- `provider` - Provider management

## Data Flow

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│   Channel   │────▶│  Message    │────▶│ Agent Loop  │
│  (Telegram, │     │    Bus      │     │             │
│   Discord)  │     │             │     │             │
└─────────────┘     └─────────────┘     └──────┬──────┘
                                               │
                     ┌─────────────────────────┼──────────┐
                     │                         │          │
                     ▼                         ▼          ▼
              ┌────────────┐           ┌───────────┐  ┌────────┐
              │   LLM      │           │   Tool    │  │Session │
              │  Provider  │◀─────────▶│  Registry │  │Manager │
              │            │           │           │  │        │
              └────────────┘           └───────────┘  └────────┘
```

## Message Lifecycle

1. **Inbound**: Channel → MessageBus → AgentLoop
2. **Processing**: AgentLoop builds context, calls LLM
3. **Tool Execution**: If LLM returns tool calls, execute and continue
4. **Outbound**: AgentLoop → MessageBus → Channel

## Configuration Flow

```
~/.nanobot/config.json
         │
         ▼
  ┌──────────────┐
  │ ConfigLoader │
  └──────┬───────┘
         │
         ▼
  ┌──────────────┐
  │    Config    │
  └──────┬───────┘
         │
    ┌────┴────┐
    │         │
    ▼         ▼
┌───────┐ ┌──────────┐
│Agents │ │Providers │
│Config │ │ Config   │
└───────┘ └──────────┘
```

## Threading Model

- **Tokio Runtime**: Multi-threaded async executor
- **Message Bus**: MPSC channels for lock-free message passing
- **Session State**: Protected by `parking_lot::Mutex`
- **Tool Registry**: Read-heavy, uses `parking_lot::Mutex`

## Error Handling

- **anyhow**: Application-level errors (CLI, commands)
- **thiserror**: Library-level errors (config, providers, tools)
- **Result propagation**: `?` operator throughout
- **Logging**: `tracing` for structured logging

## Extension Points

### Adding a Provider

1. Add `ProviderSpec` to `PROVIDERS` in `registry.rs`
2. Add config field to `ProvidersConfig` in `schema.rs`
3. Implement `LLMProvider` trait (if not OpenAI-compatible)

### Adding a Tool

1. Implement `Tool` trait
2. Register in `ToolRegistry`
3. Tool appears automatically in LLM schema

### Adding a Channel

1. Implement channel receiver (WebSocket, polling, etc.)
2. Publish to `MessageBus` as `InboundMessage`
3. Subscribe to `OutboundMessage` for sending

## Phase 1 Status

✅ Config system complete
✅ Provider registry complete
✅ Message bus complete
✅ Agent loop skeleton complete
✅ CLI framework complete

**Next**: Phase 2 - Tool implementations
