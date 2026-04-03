# QQ Channel 配置指南

## 概述

RustBot 支持通过 QQ 开放平台 Bot 进行消息交互。本文档介绍如何配置和使用 QQ Channel。

## 快速开始

### 步骤 1：创建 QQ 开放平台应用

1. 访问 [QQ 开放平台](https://bot.q.qq.com/)
2. 登录 QQ 开放平台账号
3. 进入「控制台」→「创建应用」
4. 选择应用类型（推荐选择「人工智能」或「工具效率」）
5. 填写应用信息：
   - 应用名称
   - 应用图标
   - 应用描述
6. 提交审核（审核通过后才能上线）

### 步骤 2：获取应用凭证

应用审核通过后，在应用管理页面记录以下信息：

1. **App ID** (格式：`190xxxxxxx`)
2. **Client Secret** (在应用密钥页面生成)

### 步骤 3：配置机器人能力

在应用管理页面，配置以下能力：

| 能力 | 用途 |
|-----|------|
| 消息事件 | 接收用户消息 |
| 消息发送 | 向用户/群组发送消息 |

需要订阅的事件类型：
- `C2C_MESSAGE_CREATE` - 私聊消息
- `GROUP_AT_MESSAGE_CREATE` - 群聊 @ 机器人消息

### 步骤 4：登录 RustBot Channel

```bash
# 交互式登录
rustbot channels login qq
```

按提示输入：
1. **App ID** (例如：`1903764392`)
2. **Client Secret** (例如：`e3SsIjAc4X1V0V1X4b9hGpPzaBnP2fJy`)
3. **Bot QQ 号** (可选，用于显示)

### 步骤 5：验证配置

```bash
# 查看渠道状态
rustbot channels status
```

预期输出：
```
📡 Channel Status

✅ qq
   Token: e3SsIjAc...
   App ID Configured: true
   Bot QQ: 123456789
   Access Token Cached: true
```

## 配置文件

### 方式一：通过 CLI 配置（推荐）

使用 `rustbot channels login qq` 命令会自动保存凭证到 `~/.nanobot/auth.json`。

### 方式二：手动编辑配置文件

编辑 `~/.nanobot/config.json`：

```json
{
  "channels": {
    "qq": {
      "app_id": "1903764392",
      "client_secret": "your-client-secret-here",
      "bot_qq": "123456789"
    }
  }
}
```

## 凭证存储

凭证存储在 `~/.nanobot/auth.json`（文件权限 0600）：

```json
{
  "channels": {
    "qq": {
      "app_id": "1903764392",
      "client_secret": "***",
      "bot_qq": "123456789"
    }
  }
}
```

## 使用方式

### 1. 启动 QQ Channel

```bash
# 启动 QQ 频道
rustbot channels start qq
```

启动后会：
1. 从 QQ API 获取 Access Token（有效期 2 小时）
2. 获取 WebSocket 网关地址
3. 建立 WebSocket 长连接
4. 启动心跳保活机制
5. 开始接收和发送消息

### 2. 测试消息

在 QQ 中向机器人发送消息，RustBot 会接收并处理：

- **私聊消息**：直接发送给机器人
- **群聊消息**：需要 @ 机器人

## 工作原理

### 认证流程

1. **获取 Access Token**
   - 请求：`POST https://bots.qq.com/app/getAppAccessToken`
   - 有效期：7200 秒（2 小时）
   - 自动续期：到期前 5 分钟自动刷新

2. **获取 WebSocket 网关**
   - 请求：`GET https://api.sgroup.qq.com/gateway/bot`
   - 返回 WebSocket 连接地址

3. **建立 WebSocket 连接**
   - URL 格式：`wss://api.sgroup.qq.com/websocket/?shard=0&shard_count=1`
   - 认证头：`Authorization: QQBot {access_token}`

### 心跳机制

- 心跳间隔：45 秒
- OpCode：1
- 自动重连：连接断开后自动尝试重连

### 消息事件

| 事件类型 | 说明 | 触发条件 |
|---------|------|---------|
| `C2C_MESSAGE_CREATE` | 私聊消息 | 用户私聊机器人 |
| `DIRECT_MESSAGE_CREATE` | 频道私聊 | 用户在频道私聊 |
| `GROUP_AT_MESSAGE_CREATE` | 群 @ 消息 | 用户在群聊 @ 机器人 |
| `GROUP_MESSAGE_CREATE` | 群消息 | 群内消息（需特定权限） |

## API 参考

### 获取 Access Token

```http
POST https://bots.qq.com/app/getAppAccessToken
Content-Type: application/json

{
  "appId": "1903764392",
  "clientSecret": "your-secret"
}
```

响应：
```json
{
  "access_token": "xxxxx",
  "expires_in": "7200"
}
```

### 获取网关地址

```http
GET https://api.sgroup.qq.com/gateway/bot
Authorization: QQBot {access_token}
```

响应：
```json
{
  "url": "wss://api.sgroup.qq.com/websocket/",
  "shards": 1,
  "session_start_limit": {
    "total": 1000,
    "remaining": 999,
    "reset_after": 3600000,
    "max_concurrency": 1
  }
}
```

### 发送私聊消息

```http
POST https://api.sgroup.qq.com/users/{openid}/messages
Authorization: QQBot {access_token}
Content-Type: application/json

{
  "content": "Hello from RustBot!",
  "msg_type": 0
}
```

### 发送群消息

```http
POST https://api.sgroup.qq.com/groups/{groupid}/messages
Authorization: QQBot {access_token}
Content-Type: application/json

{
  "content": "Hello from RustBot!",
  "msg_type": 0
}
```

## 故障排查

### 1. 认证失败

**错误：** `Invalid QQ credentials`

**解决：**
- 检查 App ID 和 Client Secret 是否正确
- 确认应用已通过审核并上线
- 检查应用状态是否正常

### 2. Token 获取失败

**错误：** `Failed to get access token`

**解决：**
- 检查网络连接
- 确认凭证未过期
- 检查应用权限配置

### 3. WebSocket 连接失败

**错误：** `QQ WebSocket connection failed`

**解决：**
- 检查网络是否通畅
- 确认 Access Token 有效
- 检查防火墙设置

### 4. 消息发送失败

**错误：** `QQ API error`

**解决：**
- 检查用户/群组 ID 格式
- 确认机器人已添加到目标群组
- 验证消息权限已开通

### 5. 查看日志

```bash
# 启用详细日志
rustbot channels start qq --verbose
```

## 消息类型

QQ Bot 支持多种消息类型：

### 文本消息（默认）

```json
{
  "content": "Hello World",
  "msg_type": 0
}
```

### 图片消息

```json
{
  "content": "<img url='https://example.com/image.png'>",
  "msg_type": 0
}
```

### 表情消息

```json
{
  "content": "<emoji:id=123>",
  "msg_type": 0
}
```

### Markdown 消息

```json
{
  "content": "**Bold** and *italic*",
  "msg_type": 2
}
```

## 限制说明

- **Access Token 有效期**：2 小时（自动续期）
- **消息频率限制**：遵循 QQ 开放平台频率限制
- **WebSocket 连接数**：单应用默认 1 个连接
- **消息内容长度**：文本消息最长 4096 字符

## 相关资源

- [QQ 开放平台](https://bot.q.qq.com/)
- [QQ Bot API 文档](https://bot.q.qq.com/wiki/develop/api/)
- [WebSocket 接入指南](https://bot.q.qq.com/wiki/develop/api/gateway/)
