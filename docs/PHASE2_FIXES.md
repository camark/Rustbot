# Phase 2 编译修复总结

## 修复的问题

### 1. nanobot-bus  crate

**问题**: 使用 `parking_lot::Mutex` 导致 `Send` trait 不满足

**修复**:
- 将 `parking_lot::Mutex` 改为 `tokio::sync::Mutex`
- 更新所有 `.lock()` 调用为 `.lock().await`
- 将 `try_consume_inbound` 和 `try_consume_outbound` 改为 async 函数

### 2. nanobot-providers crate

**问题**:
- `LLMProvider` trait 不可 dyn 兼容（由于 `chat_stream` 有泛型参数）
- `is_transient_error` 是静态方法
- `bytes_stream()` 方法在新版 reqwest 中已移除

**修复**:
- 将 `chat_stream` 的回调参数改为类型别名 `StreamCallback`
- 将 `is_transient_error` 移出 trait 作为独立函数
- 使用 `response.text().await?.lines()` 替代 `bytes_stream()`
- 添加 `thiserror` 依赖

### 3. nanobot-core crate

**问题**:
- 缺少 `reqwest` 依赖
- `PATH_SEPARATOR` 不存在
- 类型不匹配（`Vec<Value>` vs `Vec<Message>`）
- 所有权问题（`PathBuf` move 后使用）
- `AgentLoop` 字段不可 clone
- 递归 async 函数问题

**修复**:
- 添加 `reqwest` 依赖到 Cargo.toml
- 使用 `;` 替代 `PATH_SEPARATOR`
- 将 `Value` 转换为 `Message` 和 `ToolDefinition`
- 使用 `path.clone()` 避免 move
- 使用 `Arc` 包装共享状态（`tools`, `sessions`, `context`, `hooks`, `running`, `bus`）
- 将 `handle_response` 改为非异步函数，使用迭代替代递归
- 将 `hooks` 和 `running` 改为 `Arc<RwLock<>>` 包装

### 4. nanobot-cli crate

**问题**:
- `workspace_path` 是方法不是字段
- `config` 需要可变引用
- `io::stdout().flush()` 返回类型问题
- `AgentLoopConfig` 缺少 `tools_config` 字段
- `AgentLoop.bus` 是私有的
- `Path::to_path_buf` 类型不匹配
- `partition4` 的 `FnMut` 借用问题

**修复**:
- 使用 `config.workspace_path()` 方法调用
- 将 `config` 声明为 `mut`
- 使用 `let _ = io::stdout().flush()` 忽略错误
- 添加 `tools_config: None` 到 `AgentLoopConfig`
- 添加 `pub fn bus(&self) -> &Arc<MessageBus>` 方法
- 使用 `|p: &str| PathBuf::from(p)` 替代 `Path::to_path_buf`
- 将 `FnMut` 改为 `Fn`

## 新增依赖

```toml
# nanobot-core/Cargo.toml
reqwest.workspace = true

# nanobot-bus/Cargo.toml
thiserror.workspace = true
```

## 构建结果

```bash
cargo build --release
# Finished `release` profile [optimized] target(s) in 12.13s
```

## 验证

```bash
./target/release/rustbot --help
# RustBot - AI Assistant Framework
#  Commands: onboard, agent, gateway, status, provider, channels
```

## Phase 2 状态

所有工具已实现并编译通过：
- ✅ Shell 执行工具
- ✅ 文件系统工具（ReadFile, WriteFile, EditFile, ListDir）
- ✅ Web 搜索工具（支持多个提供商）
- ✅ Web 获取工具
