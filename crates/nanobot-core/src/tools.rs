//! Tool system for agent capabilities

pub mod shell;
pub mod fs;
pub mod web;

use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

pub use shell::{ShellTool, ShellToolConfig};
pub use fs::{ReadFileTool, WriteFileTool, EditFileTool, ListDirTool};
pub use web::{WebSearchTool, WebFetchTool, WebSearchConfig, SearchProvider};

/// Tool error types
#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("Tool not found: {0}")]
    NotFound(String),

    #[error("Invalid parameters: {0}")]
    InvalidParams(String),

    #[error("Execution error: {0}")]
    Execution(String),

    #[error("Timeout")]
    Timeout,

    #[error("Internal error: {0}")]
    Internal(String),
}

/// Result type for tool execution
pub type ToolResult<T> = Result<T, ToolError>;

/// Base tool trait
#[async_trait]
pub trait Tool: Send + Sync {
    /// Get tool name
    fn name(&self) -> &str;

    /// Get tool description
    fn description(&self) -> &str;

    /// Get parameters JSON schema
    fn parameters(&self) -> Value;

    /// Execute the tool
    async fn execute(&self, params: Value) -> ToolResult<Value>;

    /// Validate parameters
    fn validate_params(&self, params: &Value) -> Vec<String> {
        let mut errors = Vec::new();

        if let Some(obj) = params.as_object() {
            if let Some(props) = self.parameters().get("properties").and_then(|p| p.as_object()) {
                for (key, prop) in props {
                    if prop.get("required").and_then(|r| r.as_bool()).unwrap_or(false) {
                        if !obj.contains_key(key) {
                            errors.push(format!("Missing required parameter: {}", key));
                        }
                    }
                }
            }
        }

        errors
    }
}

/// Tool registry
pub struct ToolRegistry {
    tools: Mutex<HashMap<String, Arc<dyn Tool>>>,
}

impl ToolRegistry {
    /// Create a new tool registry
    pub fn new() -> Self {
        Self {
            tools: Mutex::new(HashMap::new()),
        }
    }

    /// Register a tool
    pub async fn register(&self, tool: Arc<dyn Tool>) {
        let name = tool.name().to_string();
        let mut tools = self.tools.lock().await;
        tools.insert(name, tool);
    }

    /// Register a tool wrapper (for MCP tools)
    pub async fn register_arc(&self, tool: Arc<dyn Tool>) {
        self.register(tool).await;
    }

    /// Unregister a tool
    pub async fn unregister(&self, name: &str) -> Option<Arc<dyn Tool>> {
        let mut tools = self.tools.lock().await;
        tools.remove(name)
    }

    /// Get a tool by name (returns tool name if exists)
    pub async fn contains(&self, name: &str) -> bool {
        let tools = self.tools.lock().await;
        tools.contains_key(name)
    }

    /// Check if a tool is registered
    pub async fn has(&self, name: &str) -> bool {
        let tools = self.tools.lock().await;
        tools.contains_key(name)
    }

    /// Get all tool definitions
    pub async fn get_definitions(&self) -> Vec<Value> {
        let tools = self.tools.lock().await;
        tools
            .values()
            .map(|tool| {
                serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": tool.name(),
                        "description": tool.description(),
                        "parameters": tool.parameters(),
                    }
                })
            })
            .collect()
    }

    /// Execute a tool by name
    pub async fn execute(&self, name: &str, params: Value) -> ToolResult<Value> {
        // Get tool reference and validate params while holding lock
        let validation_result = {
            let tools = self.tools.lock().await;
            let tool = tools
                .get(name)
                .ok_or_else(|| ToolError::NotFound(name.to_string()))?;

            // Validate parameters
            let errors = tool.validate_params(&params);
            if !errors.is_empty() {
                return Err(ToolError::InvalidParams(errors.join("; ")));
            }
            Ok(())
        };

        validation_result?;

        // Re-acquire lock for execution (tool may need async operations)
        let tools = self.tools.lock().await;
        let tool = tools
            .get(name)
            .ok_or_else(|| ToolError::NotFound(name.to_string()))?;

        tool.execute(params).await
    }

    /// Get list of registered tool names
    pub async fn tool_names(&self) -> Vec<String> {
        let tools = self.tools.lock().await;
        tools.keys().cloned().collect()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}
