# Ollama 配置指南

## 概述

RustBot 支持通过 Ollama 运行本地 LLM 模型。Ollama 是一个本地运行开源大语言模型的工具，支持完全离线使用，无需 API 密钥。

## 快速开始

### 步骤 1：安装 Ollama

1. 访问 [Ollama 官网](https://ollama.ai/)
2. 下载并安装适用于 Windows 的 Ollama
3. 安装完成后，Ollama 服务会自动在后台运行

### 步骤 2：下载模型

```bash
# 下载 Llama 3 模型
ollama pull llama3

# 下载 Qwen2.5 模型
ollama pull qwen2.5

# 下载 DeepSeek-R1 模型
ollama pull deepseek-r1

# 查看所有可用模型
ollama list
```

### 步骤 3：配置 RustBot

编辑 `~/.nanobot/config.json`（Windows: `C:\Users\<你的用户名>\.nanobot\config.json`）：

```json
{
  "providers": {
    "ollama": {
      "apiKey": "",
      "api_base": "http://localhost:11434/v1"
    }
  },
  "agents": {
    "defaults": {
      "model": "llama3",
      "provider": "ollama",
      "maxTokens": 8192,
      "contextWindowTokens": 65536,
      "temperature": 0.7,
      "maxToolIterations": 40,
      "timezone": "UTC"
    }
  }
}
```

### 步骤 4：验证配置

```bash
# 测试对话
rustbot agent -m "Hello, Ollama!"

# 交互模式
rustbot agent
```

## 配置选项

### 完整配置示例

```json
{
  "providers": {
    "ollama": {
      "apiKey": "",
      "api_base": "http://localhost:11434/v1"
    }
  },
  "agents": {
    "defaults": {
      "model": "llama3",
      "provider": "ollama",
      "maxTokens": 4096,
      "contextWindowTokens": 8192,
      "temperature": 0.7,
      "maxToolIterations": 30,
      "timezone": "Asia/Shanghai"
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
        "maxResults": 5
      }
    }
  }
}
```

### 配置字段说明

| 字段 | 说明 | 默认值 |
|------|------|--------|
| `providers.ollama.apiKey` | Ollama API 密钥（本地运行可留空） | `""` |
| `providers.ollama.api_base` | Ollama API 地址 | `http://localhost:11434/v1` |
| `agents.defaults.model` | 默认使用的模型名称 | `llama3` |
| `agents.defaults.provider` | 默认提供商 | `ollama` |
| `agents.defaults.maxTokens` | 最大生成 token 数 | `8192` |
| `agents.defaults.contextWindowTokens` | 上下文窗口大小 | `65536` |
| `agents.defaults.temperature` | 生成温度（0-1） | `0.7` |

## 常用模型推荐

| 模型 | 用途 | 显存需求 |
|------|------|----------|
| `llama3` | 通用对话 | ~8GB |
| `qwen2.5` | 中文优化 | ~8GB |
| `deepseek-r1` | 推理任务 | ~16GB |
| `codellama` | 代码生成 | ~8GB |
| `mistral` | 轻量级通用 | ~4GB |

## 命令示例

```bash
# 使用特定模型运行单条消息
rustbot agent -m "写一个 Python 排序函数" --model codellama

# 启动 API 服务器
rustbot api --port 8900

# 查看当前配置
cat ~/.nanobot/config.json
```

## API 服务器模式

使用 Ollama 启动 API 服务器：

```bash
# 启动 API 服务器（无认证）
rustbot api --port 8900

# 启动 API 服务器（带认证）
rustbot api --port 8900 --api-key your-secret-key
```

**测试 API:**

```bash
# 健康检查
curl http://localhost:8900/health

# 聊天（非流式）
curl -X POST http://localhost:8900/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "llama3",
    "messages": [{"role": "user", "content": "Hello!"}]
  }'
```

## 故障排查

### 1. Ollama 服务未运行

**错误：** `connection refused` 或 `failed to connect`

**解决：**
```bash
# 启动 Ollama 服务
ollama serve

# 检查服务状态
curl http://localhost:11434/api/tags
```

### 2. 模型未下载

**错误：** `model not found` 或 `404`

**解决：**
```bash
# 下载模型
ollama pull llama3

# 验证模型已下载
ollama list
```

### 3. 显存不足

**错误：** Ollama 服务崩溃或响应缓慢

**解决：**
- 使用更小的模型（如 `phi3`、`mistral`）
- 关闭其他占用显存的程序
- 考虑使用 CPU 推理（速度较慢）

### 4. 配置不生效

**解决：**
```bash
# 检查配置文件路径
echo ~/.nanobot/config.json

# 验证 JSON 格式
cat ~/.nanobot/config.json | python -m json.tool

# 重启 RustBot
taskkill //F //IM rustbot.exe
rustbot agent
```

## 高级配置

### 自定义 Ollama 地址

如果 Ollama 运行在远程服务器上：

```json
{
  "providers": {
    "ollama": {
      "apiKey": "",
      "api_base": "http://192.168.1.100:11434/v1"
    }
  }
}
```

### 启用 GPU 加速

在 Ollama 配置中启用 GPU（通常在 Ollama 服务端配置）：

```bash
# 设置环境变量
set OLLAMA_GPU_LAYER=32
ollama serve
```

### 多模型切换

在不同场景使用不同模型：

```bash
# 日常对话
rustbot agent --model llama3

# 代码任务
rustbot agent --model codellama

# 中文任务
rustbot agent --model qwen2.5
```

## 性能优化

1. **选择合适的模型**：根据任务选择专用模型
2. **调整上下文窗口**：较小的上下文窗口可以节省显存
3. **使用 GPU**：确保 Ollama 配置为使用 GPU 加速
4. **批量请求**：API 模式下可以批量处理请求

## 相关资源

- [Ollama 官方文档](https://ollama.ai/)
- [Ollama 模型库](https://ollama.ai/library)
- [RustBot 配置指南](./RUNNABLE_VERSION.md)
- [RustBot 架构说明](./ARCHITECTURE.md)
