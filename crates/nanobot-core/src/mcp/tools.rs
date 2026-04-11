//! MCP Tools Integration
//!
//! This module provides integration between MCP tools and RustBot's internal
//! tool system, allowing MCP-discovered tools to be called through the standard
//! Tool trait.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::RwLock;
use tracing::{debug, info};

use super::client::McpClient;
use super::protocol::Tool as McpTool;
use crate::tools::{ToolResult, ToolError};

/// MCP Tool wrapper that implements the internal Tool trait
pub struct McpToolWrapper {
    /// Original MCP tool definition
    pub mcp_tool: McpTool,
    /// Reference to the MCP client
    client: Arc<McpClient>,
}

impl McpToolWrapper {
    /// Create a new tool wrapper
    pub fn new(mcp_tool: McpTool, client: Arc<McpClient>) -> Self {
        Self { mcp_tool, client }
    }

    /// Get the tool name
    pub fn name(&self) -> &str {
        &self.mcp_tool.name
    }

    /// Get the tool description
    pub fn description(&self) -> &str {
        &self.mcp_tool.description
    }

    /// Get the input schema (MCP uses input_schema, internal uses parameters)
    pub fn input_schema(&self) -> &Value {
        &self.mcp_tool.input_schema
    }

    /// Call the MCP tool with the given arguments
    pub async fn call(&self, arguments: Value) -> Result<ToolCallResult> {
        debug!(
            "Calling MCP tool: {} with arguments: {}",
            self.name(),
            arguments
        );

        let result = self
            .client
            .call_tool(&self.mcp_tool.name, arguments)
            .await
            .context("MCP tool call failed")?;

        // Convert protocol::ToolCallResult to tools::ToolCallResult
        Ok(result.into())
    }
}

/// Result from an MCP tool call
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallResult {
    /// Content items returned by the tool
    pub content: Vec<ToolCallContent>,
    /// Whether the tool call resulted in an error
    pub is_error: Option<bool>,
}

/// Content item from a tool call
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToolCallContent {
    /// Text content
    Text { text: String },
    /// Image content (base64 encoded)
    Image { data: String, mime_type: String },
    /// Resource reference
    Resource { resource: Value },
}

impl From<super::protocol::ToolCallResult> for ToolCallResult {
    fn from(result: super::protocol::ToolCallResult) -> Self {
        Self {
            content: result
                .content
                .into_iter()
                .map(|c| match c {
                    super::protocol::ToolCallContent::Text { text } => ToolCallContent::Text { text },
                    super::protocol::ToolCallContent::Image { data, mime_type } => {
                        ToolCallContent::Image { data, mime_type }
                    }
                    super::protocol::ToolCallContent::Resource { resource } => {
                        ToolCallContent::Resource { resource }
                    }
                })
                .collect(),
            is_error: result.is_error,
        }
    }
}

/// Registry for MCP tools
pub struct McpToolRegistry {
    /// MCP client connection
    client: Arc<McpClient>,
    /// Cached tools
    tools: Arc<RwLock<HashMap<String, Arc<McpToolWrapper>>>>,
}

impl McpToolRegistry {
    /// Create a new MCP tool registry
    pub fn new(client: Arc<McpClient>) -> Self {
        Self {
            client,
            tools: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Discover and cache tools from the MCP server
    pub async fn refresh(&self) -> Result<Vec<Arc<McpToolWrapper>>> {
        info!("Refreshing MCP tool registry...");

        // Discover tools from server
        let mcp_tools = self
            .client
            .discover_tools()
            .await
            .context("Failed to discover tools")?;

        // Build tool cache
        let mut tools_map = self.tools.write().await;
        tools_map.clear();

        let wrappers: Vec<Arc<McpToolWrapper>> = mcp_tools
            .into_iter()
            .map(|tool| {
                let wrapper = Arc::new(McpToolWrapper::new(tool, self.client.clone()));
                tools_map.insert(wrapper.name().to_string(), wrapper.clone());
                wrapper
            })
            .collect();

        info!("MCP tool registry refreshed: {} tools available", wrappers.len());

        Ok(wrappers)
    }

    /// Get a tool by name
    pub async fn get_tool(&self, name: &str) -> Option<Arc<McpToolWrapper>> {
        self.tools.read().await.get(name).cloned()
    }

    /// List all available tools
    pub async fn list_tools(&self) -> Vec<Arc<McpToolWrapper>> {
        self.tools
            .read()
            .await
            .values()
            .cloned()
            .collect()
    }

    /// Check if a tool is available
    pub async fn has_tool(&self, name: &str) -> bool {
        self.tools.read().await.contains_key(name)
    }

    /// Call a tool by name
    pub async fn call_tool(&self, name: &str, arguments: Value) -> Result<ToolCallResult> {
        let tool = self
            .get_tool(name)
            .await
            .ok_or_else(|| anyhow::anyhow!("Tool not found: {}", name))?;

        tool.call(arguments).await
    }
}

/// Helper to convert MCP tools to internal Tool trait objects
pub fn create_tool_adaptor(
    tool: Arc<McpToolWrapper>,
) -> impl crate::tools::Tool {
    McpToolAdaptor { tool }
}

/// Tool adaptor that bridges MCP tools to the internal Tool trait
pub struct McpToolAdaptor {
    tool: Arc<McpToolWrapper>,
}

#[async_trait::async_trait]
impl crate::tools::Tool for McpToolAdaptor {
    fn name(&self) -> &str {
        self.tool.name()
    }

    fn description(&self) -> &str {
        self.tool.description()
    }

    fn parameters(&self) -> Value {
        self.tool.input_schema().clone()
    }

    async fn execute(&self, arguments: Value) -> ToolResult<Value> {
        match self.tool.call(arguments).await {
            Ok(result) => {
                // Convert MCP result to internal ToolResult
                let mut content = String::new();
                for item in result.content {
                    match item {
                        ToolCallContent::Text { text } => {
                            content.push_str(&text);
                            content.push('\n');
                        }
                        ToolCallContent::Image { data: _, mime_type } => {
                            content.push_str(&format!("[Image: {}]", mime_type));
                            content.push('\n');
                        }
                        ToolCallContent::Resource { resource } => {
                            content.push_str(&format!("[Resource: {}]", resource));
                            content.push('\n');
                        }
                    }
                }

                if result.is_error.unwrap_or(false) {
                    Err(ToolError::Execution(content))
                } else {
                    Ok(serde_json::json!({ "content": content.trim() }))
                }
            }
            Err(e) => Err(ToolError::Execution(e.to_string())),
        }
    }
}

/// Integration point for MCP tools with the agent system
pub struct McpToolIntegration {
    registry: Arc<McpToolRegistry>,
}

impl McpToolIntegration {
    /// Create a new MCP tool integration
    pub fn new(client: Arc<McpClient>) -> Self {
        Self {
            registry: Arc::new(McpToolRegistry::new(client)),
        }
    }

    /// Initialize the integration (discover tools)
    pub async fn initialize(&self) -> Result<Vec<Arc<McpToolWrapper>>> {
        self.registry.refresh().await
    }

    /// Get the tool registry
    pub fn registry(&self) -> Arc<McpToolRegistry> {
        self.registry.clone()
    }

    /// Convert all MCP tools to internal Tool trait objects
    pub async fn get_all_tools(&self) -> Vec<Arc<dyn crate::tools::Tool>> {
        let wrappers = self.registry.list_tools().await;
        let tools: Vec<Arc<dyn crate::tools::Tool>> = wrappers
            .into_iter()
            .map(|w| Arc::new(create_tool_adaptor(w)) as Arc<dyn crate::tools::Tool>)
            .collect();
        tools
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::client::{McpClient, McpClientConfig};
    use serde_json::json;

    #[tokio::test]
    async fn test_tool_registry_empty() {
        let client = Arc::new(McpClient::default());
        let registry = McpToolRegistry::new(client);

        let tools = registry.list_tools().await;
        assert!(tools.is_empty());

        let has_tool = registry.has_tool("nonexistent").await;
        assert!(!has_tool);
    }

    #[tokio::test]
    async fn test_tool_wrapper_creation() {
        let mcp_tool = McpTool {
            name: "test_tool".to_string(),
            description: "A test tool".to_string(),
            input_schema: json!({"type": "object"}),
        };

        let client = Arc::new(McpClient::default());
        let wrapper = McpToolWrapper::new(mcp_tool, client);

        assert_eq!(wrapper.name(), "test_tool");
        assert_eq!(wrapper.description(), "A test tool");
    }
}
