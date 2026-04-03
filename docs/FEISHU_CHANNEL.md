# Feishu Channel 配置指南

## 概述

RustBot 支持通过飞书（Feishu）机器人进行消息交互。本文档介绍如何配置和使用 Feishu Channel。

## 快速开始

### 步骤 1：创建飞书自建应用

1. 访问 [飞书开放平台](https://open.feishu.cn/)
2. 登录企业管理员账号
3. 进入「企业管理」→「应用开发」→「自建应用」
4. 点击「创建应用」
5. 填写应用名称和描述
6. 创建后记录以下信息：
   - **App ID** (格式：`cli_xxxxx`)
   - **App Secret**

### 步骤 2：配置应用权限

在应用管理页面，添加以下权限：

| 权限名称 | 权限标识 | 用途 |
|---------|---------|------|
| 发送消息 | `im:message` | 向用户/群组发送消息 |
| 读取会话信息 | `im:chat` | 获取会话详情 |
| 事件订阅 | `im:message.receive` | 接收用户消息 |

### 步骤 3：配置机器人

1. 在应用管理页面，进入「机器人」功能
2. 点击「添加机器人」
3. 设置机器人头像和名称
4. 复制 **Verification Token**（用于验证 webhook）

### 步骤 4：启用事件订阅

1. 进入「事件订阅」页面
2. 启用事件订阅
3. 配置订阅地址（如果使用 webhook 模式）
4. 订阅 `im.message.receive_v1` 事件

### 步骤 5：登录 RustBot Channel

```bash
# 交互式登录
rustbot channels login feishu
```

按提示输入：
1. **App ID** (例如：`cli_a1b2c3d4e5f6`)
2. **App Secret** (例如：`abcdefghijklmnopqrstuvwxyz123456`)
3. **Verification Token** (例如：`xxxx-xxxx-xxxx`)

### 步骤 6：验证配置

```bash
# 查看渠道状态
rustbot channels status
```

预期输出：
```
📡 Channel Status

✅ feishu
   Token: abcdefgh...
   App ID Configured: true
   Access Token Cached: true
```

## 配置文件

### 方式一：通过 CLI 配置（推荐）

使用 `rustbot channels login feishu` 命令会自动保存凭证到 `~/.nanobot/auth.json`。

### 方式二：手动编辑配置文件

编辑 `~/.nanobot/config.json`：

```json
{
  "channels": {
    "feishu": {
      "app_id": "cli_a1b2c3d4e5f6",
      "app_secret": "your-app-secret-here",
      "verification_token": "your-verification-token-here"
    }
  }
}
```

## 凭证存储

凭证存储在 `~/.nanobot/auth.json`（文件权限 0600）：

```json
{
  "channels": {
    "feishu": {
      "app_id": "cli_xxxxx",
      "app_secret": "***",
      "verification_token": "xxxx"
    }
  }
}
```

## 使用方式

### 1. 启动 Gateway 服务

```bash
# 启动网关服务（监听飞书 webhook）
rustbot gateway
```

### 2. 配置飞书 Webhook URL

在飞书开放平台的「事件订阅」页面，配置 Webhook URL：

```
http://your-server-ip:18790/feishu/webhook
```

### 3. 测试消息

在飞书中向机器人发送消息，RustBot 会接收并处理。

## API 参考

### 获取 Tenant Access Token

```http
POST https://open.feishu.cn/open-apis/auth/v3/tenant_access_token/internal
Content-Type: application/json

{
  "app_id": "cli_xxxxx",
  "app_secret": "your-secret"
}
```

响应：
```json
{
  "code": 0,
  "tenant_access_token": "xxxxx",
  "expire": 7140
}
```

### 发送消息

```http
POST https://open.feishu.cn/open-apis/im/v1/messages
Authorization: Bearer <tenant_access_token>
Content-Type: application/json

{
  "receive_id": "open_id_xxx",
  "msg_type": "text",
  "content": {
    "text": "Hello from RustBot!"
  }
}
```

## 故障排查

### 1. 认证失败

**错误：** `Invalid Feishu credentials`

**解决：**
- 检查 App ID 和 App Secret 是否正确
- 确认应用已发布（未发布的应用某些 API 不可用）

### 2. Token 获取失败

**错误：** `Failed to fetch Feishu access token`

**解决：**
- 检查网络连接
- 确认应用权限已正确配置

### 3. 消息发送失败

**错误：** `Feishu send message failed`

**解决：**
- 检查用户/群组 ID 是否正确
- 确认机器人已添加到目标群组
- 验证消息权限已开通

### 4. 查看日志

```bash
# 启用详细日志
rustbot gateway --verbose
```

## 高级配置

### 自定义消息格式

Feishu 支持多种消息类型：

**文本消息（默认）：**
```json
{
  "msg_type": "text",
  "content": { "text": "Hello" }
}
```

**富文本消息：**
```json
{
  "msg_type": "post",
  "content": {
    "post": {
      "zh_cn": {
        "title": "标题",
        "content": [[
          {"tag": "text", "text": "内容"}
        ]]
      }
    }
  }
}
```

**交互式卡片：**
```json
{
  "msg_type": "interactive",
  "card": { ... }
}
```

### Webhook 处理

Webhook 接收器需要处理以下事件：

1. **URL 验证**：飞书会发送验证请求
2. **消息接收**：`im.message.receive_v1` 事件

## 相关资源

- [飞书开放平台](https://open.feishu.cn/)
- [飞书 API 文档](https://open.feishu.cn/document/ukTMukTMukTM/ucjM1UjL3YDM10yN1ATN)
- [RustBot 配置指南](./RUNNABLE_VERSION.md)
