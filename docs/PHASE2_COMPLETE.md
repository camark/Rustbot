# Phase 2 完成总结

## 实现的工具

### 1. Shell 执行工具 (`tools/shell.rs`)

**功能**: 执行系统命令

**特性**:
- 安全命令检查（阻止危险命令如 `rm -rf /`）
- 超时控制（默认 60 秒）
- 工作目录限制
- PATH 环境变量扩展

**使用方法**:
```json
{
  "name": "exec",
  "arguments": {
    "command": "ls -la"
  }
}
```

---

### 2. 文件系统工具 (`tools/fs.rs`)

#### ReadFileTool
- 读取文件内容
- 支持相对路径和绝对路径
- 工作目录限制
- 路径遍历保护

```json
{
  "name": "read_file",
  "arguments": {
    "path": "README.md"
  }
}
```

#### WriteFileTool
- 写入文件内容
- 自动创建父目录
- 覆盖已存在文件

```json
{
  "name": "write_file",
  "arguments": {
    "path": "test.txt",
    "content": "Hello, World!"
  }
}
```

#### EditFileTool
- 搜索并替换文件内容
- 支持单次替换或全部替换
- 返回替换次数

```json
{
  "name": "edit_file",
  "arguments": {
    "path": "test.txt",
    "search": "Hello",
    "replace": "Hi",
    "all": false
  }
}
```

#### ListDirTool
- 列出目录内容
- 返回文件和子目录
- 标识类型（file/directory）

```json
{
  "name": "list_dir",
  "arguments": {
    "path": "."
  }
}
```

---

### 3. Web 工具 (`tools/web.rs`)

#### WebSearchTool
**支持的搜索引擎**:
- **Brave Search** (默认) - 需要 API Key
- **Jina Search** - 需要 API Key
- **Tavily** - 需要 API Key
- **SearXNG** - 自建实例
- **DuckDuckGo** - 无需 API Key（HTML 爬取）

**配置**:
```rust
WebSearchConfig {
    provider: SearchProvider::Brave,
    api_key: "xxx".to_string(),
    max_results: 5,
}
```

**使用**:
```json
{
  "name": "web_search",
  "arguments": {
    "query": "Rust programming"
  }
}
```

**返回**:
```json
{
  "query": "Rust programming",
  "results": [
    {
      "title": "Rust Programming Language",
      "url": "https://www.rust-lang.org/",
      "snippet": "Rust is a multi-paradigm..."
    }
  ],
  "count": 5
}
```

#### WebFetchTool
- 获取网页内容
- 超时控制（30 秒）
- 内容截断（>50KB 自动截断）
- 支持代理配置

**使用**:
```json
{
  "name": "web_fetch",
  "arguments": {
    "url": "https://example.com"
  }
}
```

---

## 工具注册

所有工具在 `AgentLoop::register_default_tools()` 中自动注册：

```rust
fn register_default_tools(&mut self) {
    // Shell tool
    self.tools.register(Box::new(ShellTool::new(&self.workspace, ShellToolConfig::default())));

    // Filesystem tools
    let allowed_dir = Some(self.workspace.clone());
    self.tools.register(Box::new(ReadFileTool::new(&self.workspace, allowed_dir.clone())));
    self.tools.register(Box::new(WriteFileTool::new(&self.workspace, allowed_dir.clone())));
    self.tools.register(Box::new(EditFileTool::new(&self.workspace, allowed_dir.clone())));
    self.tools.register(Box::new(ListDirTool::new(&self.workspace, allowed_dir.clone())));

    // Web tools
    self.tools.register(Box::new(WebSearchTool::new(WebSearchConfig::default())));
    self.tools.register(Box::new(WebFetchTool::new(None)));
}
```

---

## 新增依赖

`crates/nanobot-core/Cargo.toml`:
```toml
urlencoding = "2.1"  # URL 编码支持
```

---

## 安全特性

### Shell 工具安全
- 阻止危险命令模式：
  - `rm -rf /`
  - `format c:`
  - `dd if=/dev/zero`
  - `:(){:|:&};:` (fork bomb)
  - `shutdown`, `reboot` 等

### 文件系统安全
- 工作目录限制（可选）
- 路径遍历检测
- 规范化路径验证

### Web 工具安全
- URL 协议验证（仅允许 http/https）
- 请求超时
- 响应大小限制

---

## 测试

运行工具测试：
```bash
cargo test -p nanobot-core
```

---

## 下一步 (Phase 3)

1. **会话持久化增强**
   - 磁盘存储优化
   - 会话过期策略

2. **记忆整合**
   - 自动摘要生成
   - 上下文窗口管理
   - Token 计数优化

3. **消息工具**
   - 主动发送消息
   - 广播支持

---

## 文件清单

```
crates/nanobot-core/src/
├── tools/
│   ├── shell.rs    # Shell 执行
│   ├── fs.rs       # 文件系统
│   └── web.rs      # Web 搜索/获取
├── agent.rs        # (已更新) 工具注册
└── tools.rs        # (已更新) 模块导出
```
