# RustBot 可运行版本构建完成

**构建时间:** 2026-04-03
**版本:** v0.1.0
**二进制大小:** 12MB

## 快速开始

### 1. 初始化配置

```bash
# 首次运行，初始化配置
./target/release/rustbot onboard
```

### 2. 配置 API Key

编辑 `~/.nanobot/config.json`：

```json
{
  "providers": {
    "openrouter": {
      "apiKey": "sk-or-v1-your-api-key-here"
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

### 3. 开始对话

```bash
# 单条消息
./target/release/rustbot agent -m "Hello, RustBot!"

# 交互模式
./target/release/rustbot agent

# 带日志
./target/release/rustbot agent --logs
```

## 可用命令

### 核心命令

| 命令 | 描述 | 示例 |
|------|------|------|
| `onboard` | 初始化配置 | `rustbot onboard` |
| `agent` | 与 AI 对话 | `rustbot agent -m "Hello"` |
| `status` | 显示状态 | `rustbot status` |

### API 服务器

```bash
# 启动 API 服务器（无认证）
./target/release/rustbot api --port 8900

# 启动 API 服务器（带认证）
./target/release/rustbot api --port 8900 --api-key your-secret-key
```

**API 端点:**
- `GET /health` - 健康检查
- `GET /v1/models` - 列出可用模型
- `POST /v1/chat/completions` - 聊天完成（支持 SSE 流式）

**测试 API:**
```bash
# 健康检查
curl http://localhost:8900/health

# 列出模型
curl http://localhost:8900/v1/models

# 聊天（非流式）
curl -X POST http://localhost:8900/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer your-api-key" \
  -d '{
    "model": "rustbot/default",
    "messages": [{"role": "user", "content": "Hello!"}]
  }'
```

### 定时任务 (Cron)

```bash
# 添加定时任务（每 5 秒执行）
./target/release/rustbot cron add "cleanup" "*/5 * * * * *"

# 添加定时任务（每小时执行）
./target/release/rustbot cron add "hourly" "0 0 * * * *"

# 添加定时任务（每天午夜执行）
./target/release/rustbot cron add "daily" "0 0 0 * * *"

# 列出所有任务
./target/release/rustbot cron list

# 手动执行任务
./target/release/rustbot cron run "cleanup"

# 删除任务
./target/release/rustbot cron remove "cleanup"
```

### 服务管理

```bash
# 查看服务状态
./target/release/rustbot services status

# 启动服务
./target/release/rustbot services start cron
./target/release/rustbot services start heartbeat

# 停止服务
./target/release/rustbot services stop cron
./target/release/rustbot services stop heartbeat
```

### 渠道管理 (Channels)

```bash
# 登录渠道
./target/release/rustbot channels login telegram
./target/release/rustbot channels login feishu

# 查看状态
./target/release/rustbot channels status
```

**Feishu 配置示例** (`~/.nanobot/config.json`):
```json
{
  "channels": {
    "feishu": {
      "app_id": "cli_a1b2c3d4e5f6",
      "app_secret": "your-app-secret",
      "verification_token": "your-verification-token"
    }
  }
}
```

**Telegram 配置示例**:
```json
{
  "channels": {
    "telegram": {
      "bot_token": "123456:ABC-DEF1234ghIkl-zyx57W2v1u123ew11"
    }
  }
}
```

## 配置目录结构

```
~/.nanobot/
├── config.json          # 主配置文件
├── auth.json            # 渠道认证信息（0600 权限）
├── workspace/           # 工作目录
├── sessions/            # 会话存储
└── cron.json            # 定时任务配置
```

## 完整配置示例

```json
{
  "providers": {
    "openrouter": {
      "apiKey": "sk-or-v1-your-key"
    },
    "anthropic": {
      "apiKey": "sk-ant-your-key"
    }
  },
  "agents": {
    "defaults": {
      "model": "anthropic/claude-opus-4-5",
      "provider": "openrouter",
      "temperature": 0.7,
      "contextWindowTokens": 65536
    }
  },
  "api": {
    "host": "127.0.0.1",
    "port": 8900
  },
  "channels": {
    "telegram": {
      "bot_token": "123456:ABC-DEF1234ghIkl-zyx57W2v1u123ew11"
    },
    "feishu": {
      "app_id": "cli_xxx",
      "app_secret": "xxx",
      "verification_token": "xxx"
    }
  },
  "tools": {
    "exec": {
      "enable": true,
      "timeout": 60
    },
    "web": {
      "search": {
        "provider": "brave",
        "apiKey": "your-search-api-key",
        "maxResults": 5
      }
    }
  }
}
```

## 故障排查

### 1. 查看日志

```bash
# 启用详细日志
./target/release/rustbot agent --verbose -m "test"
```

### 2. 检查配置

```bash
# 查看配置文件位置
cat ~/.nanobot/config.json

# 验证 JSON 格式
cat ~/.nanobot/config.json | python -m json.tool
```

### 3. 测试连接

```bash
# 测试 API 健康
curl http://localhost:8900/health
```

## 性能指标

| 指标 | 值 |
|------|-----|
| 二进制大小 | ~12MB |
| 启动时间 | <100ms |
| 内存占用 | ~20MB (空闲) |
| 测试通过率 | 100% (38/38) |

## 支持的 LLM 提供商

- OpenRouter ✅
- Anthropic ✅
- OpenAI ✅
- DeepSeek ✅
- Azure OpenAI ✅
- Ollama (本地) ✅
- vLLM (本地) ✅
- Groq ✅
- Moonshot ✅
- Gemini ✅
- Zhipu ✅
- DashScope ✅

## 下一步

1. 配置你的 API Key
2. 运行 `rustbot agent -m "Hello!"` 测试
3. 探索更多功能：API 服务器、定时任务、渠道集成
