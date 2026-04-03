//! Agent hooks for lifecycle events

use async_trait::async_trait;
use nanobot_providers::{LLMResponse, ToolCall};

/// Hook context for agent lifecycle events
#[derive(Debug, Clone)]
pub struct HookContext {
    pub session_key: String,
    pub channel: String,
    pub chat_id: String,
    pub response: Option<LLMResponse>,
    pub tool_calls: Vec<ToolCall>,
    pub usage: Option<serde_json::Value>,
}

/// Agent lifecycle hook trait
#[async_trait]
pub trait AgentHook: Send + Sync {
    /// Called before each iteration
    async fn before_iteration(&self, _ctx: &HookContext) {}

    /// Called during streaming (if supported)
    async fn on_stream(&self, _ctx: &HookContext, _delta: &str) {}

    /// Called when streaming ends
    async fn on_stream_end(&self, _ctx: &HookContext, _resuming: bool) {}

    /// Called before tool execution
    async fn before_execute_tools(&self, _ctx: &HookContext) {}

    /// Called after each iteration
    async fn after_iteration(&self, _ctx: &HookContext) {}

    /// Finalize content before sending
    fn finalize_content(&self, _ctx: &HookContext, content: Option<String>) -> Option<String> {
        content
    }

    /// Check if this hook wants streaming
    fn wants_streaming(&self) -> bool {
        false
    }
}

/// Composite hook that runs multiple hooks
pub struct CompositeHook {
    hooks: Vec<Box<dyn AgentHook>>,
}

impl CompositeHook {
    pub fn new(hooks: Vec<Box<dyn AgentHook>>) -> Self {
        Self { hooks }
    }

    pub fn add(&mut self, hook: Box<dyn AgentHook>) {
        self.hooks.push(hook);
    }
}

#[async_trait]
impl AgentHook for CompositeHook {
    async fn before_iteration(&self, ctx: &HookContext) {
        for hook in &self.hooks {
            hook.before_iteration(ctx).await;
        }
    }

    async fn on_stream(&self, ctx: &HookContext, delta: &str) {
        for hook in &self.hooks {
            hook.on_stream(ctx, delta).await;
        }
    }

    async fn on_stream_end(&self, ctx: &HookContext, resuming: bool) {
        for hook in &self.hooks {
            hook.on_stream_end(ctx, resuming).await;
        }
    }

    async fn before_execute_tools(&self, ctx: &HookContext) {
        for hook in &self.hooks {
            hook.before_execute_tools(ctx).await;
        }
    }

    async fn after_iteration(&self, ctx: &HookContext) {
        for hook in &self.hooks {
            hook.after_iteration(ctx).await;
        }
    }

    fn finalize_content(&self, ctx: &HookContext, content: Option<String>) -> Option<String> {
        let mut result = content;
        for hook in &self.hooks {
            result = hook.finalize_content(ctx, result);
        }
        result
    }

    fn wants_streaming(&self) -> bool {
        self.hooks.iter().any(|h| h.wants_streaming())
    }
}
