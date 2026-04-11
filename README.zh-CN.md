# RustBot

🐈 超轻量级个人 AI 助手框架（Rust 实现）

这是 [nanobot](https://github.com/HKUDS/nanobot) 的完整 Rust 重写版本，专为以下目标设计：

- **高性能**：比 Python 快 10-100 倍
- **低内存**：无 GC，最小内存占用
- **单一二进制**：易于部署，无需依赖
- **类型安全**：编译时保证

## 项目结构

```
RustBot/
├── Cargo.toml              # 工作空间根目录
├── crates/
│   ├── nanobot-config/     # 配置系统
│   ├── nanobot-providers/  # LLM 提供者
│   ├── nanobot-bus/        # 消息总线
│   ├── nanobot-core/       # 代理引擎
│   └── nanobot-cli/        # 命令行接口
└── docs/
```

## 快速开始

### 构建

```bash
cargo build --release
```

### 初始化

```bash
./target/release/rustbot onboard
```

### 聊天

```bash
./target/release/rustbot agent -m "你好！"
```

### 交互模式

```bash
./target/release/rustbot agent
```

### 状态

```bash
./target/release/rustbot status
```

## 配置

配置文件：`~/.nanobot/config.json`

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

## 支持的 LLM 提供者

| 提供者 | 状态 |
|----------|--------|
| OpenRouter | ✅ |
| Anthropic | ✅ |
| OpenAI | ✅ |
| DeepSeek | ✅ |
| Azure OpenAI | ✅ |
| Ollama (本地) | ✅ |
| vLLM (本地) | ✅ |
| Groq | ✅ |
| Moonshot | ✅ |
| Gemini | ✅ |
| Zhipu | ✅ |
| DashScope | ✅ |

## 功能特性

### ✅ 已完成

- **核心基础设施**：配置系统、提供者注册表、消息总线、CLI 框架
- **工具实现**：Shell 执行、文件系统工具、网络搜索、网络抓取
- **内存与会话**：会话持久化、内存清理、上下文窗口管理
- **频道集成**：Telegram、Discord、飞书频道连接器
- **服务系统**：Cron 服务、心跳服务、OpenAI 兼容 API 服务器
- **高级功能**：MCP (模型上下文协议)、子代理系统、技能系统

## 开发

###  prerequisites

- Rust 1.75+ (stable)
- Tokio 运行时

> **Ubuntu 用户注意：安装 protoc**
>
> 如果在构建时遇到 protoc 相关错误，请安装 Protocol Buffers 编译器：
>
> ```bash
> # Ubuntu/Debian
> sudo apt-get update && sudo apt-get install -y protobuf-compiler
>
> # 验证安装
> protoc --version
> ```

### 运行测试

```bash
cargo test
```

### 运行 Clippy

```bash
cargo clippy --all-targets
```

### 格式化代码

```bash
cargo fmt --all
```

## CLI 命令

| 命令 | 描述 |
|------|------|
| `rustbot onboard` | 初始化配置 |
| `rustbot agent -m "消息"` | 发送单条消息 |
| `rustbot agent` | 交互模式 |
| `rustbot api --port 8900` | 启动 API 服务器 |
| `rustbot cron list` | 列出定时任务 |
| `rustbot channels login <频道>` | 登录频道 |
| `rustbot channels status` | 查看频道状态 |
| `rustbot channels start <频道>` | 启动频道 |
| `rustbot channels stop <频道>` | 停止频道 |
| `rustbot mcp list` | 列出 MCP 服务器 |

## 许可证

MIT 许可证 - 与原始 nanobot 项目相同。

## 致谢

本项目的灵感来源于 [nanobot](https://github.com/HKUDS/nanobot)，一个超轻量级 Python AI 助手框架。
