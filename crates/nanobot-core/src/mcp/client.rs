//! MCP Client Implementation
//!
//! This module provides the main MCP client interface for connecting to external
//! MCP tool servers and discovering/calling tools.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use serde_json::json;
use tokio::sync::RwLock;
use tracing::{debug, info};

use super::protocol::{
    ClientCapabilities, InitializeParams, InitializeResult, JsonRpcRequest, Tool, ToolCallRequest, ToolCallResult, ToolsCapability, ToolsListResult,
    MCP_PROTOCOL_VERSION, rustbot_client_info,
};
use super::transport::{create_transport, McpTransport, TransportConfig};

/// MCP Client configuration
#[derive(Debug, Clone)]
pub struct McpClientConfig {
    /// Transport configuration
    pub transport: TransportConfig,
    /// Connection timeout in seconds
    pub timeout_secs: u64,
    /// Enable automatic reconnection
    pub auto_reconnect: bool,
    /// Reconnection delay in seconds
    pub reconnect_delay_secs: u64,
}

impl Default for McpClientConfig {
    fn default() -> Self {
        Self {
            transport: TransportConfig::Stdio {
                command: "npx".to_string(),
                args: vec!["-y".to_string(), "@modelcontextprotocol/server".to_string()],
                env: vec![],
            },
            timeout_secs: 30,
            auto_reconnect: true,
            reconnect_delay_secs: 5,
        }
    }
}

/// MCP Client for connecting to external tool servers
pub struct McpClient {
    config: McpClientConfig,
    transport: Arc<RwLock<Option<Arc<dyn McpTransport>>>>,
    initialized: Arc<AtomicBool>,
    server_info: Arc<RwLock<Option<ServerInfo>>>,
    tools: Arc<RwLock<Vec<Tool>>>,
    running: Arc<AtomicBool>,
}

/// Server information after initialization
#[derive(Debug, Clone)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
    pub protocol_version: String,
}

impl McpClient {
    /// Create a new MCP client with the given configuration
    pub fn new(config: McpClientConfig) -> Self {
        Self {
            config,
            transport: Arc::new(RwLock::new(None)),
            initialized: Arc::new(AtomicBool::new(false)),
            server_info: Arc::new(RwLock::new(None)),
            tools: Arc::new(RwLock::new(Vec::new())),
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Create a client with stdio transport
    pub fn stdio(command: String, args: Vec<String>, env: Vec<(String, String)>) -> Self {
        let config = McpClientConfig {
            transport: TransportConfig::Stdio {
                command,
                args,
                env,
            },
            ..Default::default()
        };
        Self::new(config)
    }

    /// Create a client with SSE transport
    #[cfg(feature = "http-client")]
    pub fn sse(url: String, headers: Vec<(String, String)>) -> Self {
        let config = McpClientConfig {
            transport: TransportConfig::Sse { url, headers },
            ..Default::default()
        };
        Self::new(config)
    }

    /// Connect to the MCP server and perform initialization handshake
    pub async fn connect(&self) -> Result<()> {
        if self.is_connected().await {
            info!("MCP client already connected");
            return Ok(());
        }

        info!("Connecting to MCP server...");

        // Create transport
        let transport = create_transport(self.config.transport.clone())
            .context("Failed to create transport")?;

        // Connect transport
        transport
            .connect()
            .await
            .context("Failed to connect transport")?;

        // Store transport
        *self.transport.write().await = Some(transport);

        // Perform initialization handshake
        self.initialize().await.context("Initialization failed")?;

        self.initialized.store(true, Ordering::SeqCst);
        self.running.store(true, Ordering::SeqCst);

        info!("MCP client connected and initialized");
        Ok(())
    }

    /// Disconnect from the MCP server
    pub async fn disconnect(&self) -> Result<()> {
        self.running.store(false, Ordering::SeqCst);
        self.initialized.store(false, Ordering::SeqCst);

        let mut transport_guard = self.transport.write().await;
        if let Some(transport) = transport_guard.take() {
            let _ = transport.disconnect().await;
        }

        *self.server_info.write().await = None;
        self.tools.write().await.clear();

        info!("MCP client disconnected");
        Ok(())
    }

    /// Check if client is connected and initialized
    pub async fn is_connected(&self) -> bool {
        self.running.load(Ordering::SeqCst)
            && self.initialized.load(Ordering::SeqCst)
            && self.transport.read().await.is_some()
    }

    /// Perform MCP initialization handshake
    async fn initialize(&self) -> Result<()> {
        let transport = self
            .transport
            .read()
            .await
            .clone()
            .ok_or_else(|| anyhow::anyhow!("Transport not available"))?;

        // Build initialize request
        let params = InitializeParams {
            protocol_version: MCP_PROTOCOL_VERSION.to_string(),
            capabilities: ClientCapabilities {
                tools: Some(ToolsCapability {
                    list_changed: Some(true),
                }),
            },
            client_info: rustbot_client_info(),
        };

        let request = JsonRpcRequest::new("initialize", serde_json::to_value(&params)?)
            .with_id(serde_json::json!(1));

        debug!("Sending MCP initialize request");

        let response = transport
            .send_request(request)
            .await
            .context("Failed to send initialize request")?;

        if let Some(result) = response.result {
            let init_result: InitializeResult =
                serde_json::from_value(result).context("Failed to parse initialize result")?;

            info!(
                "MCP server initialized: name={}, version={}, protocol={}",
                init_result.server_info.name,
                init_result.server_info.version,
                init_result.protocol_version
            );

            // Store server info
            *self.server_info.write().await = Some(ServerInfo {
                name: init_result.server_info.name,
                version: init_result.server_info.version,
                protocol_version: init_result.protocol_version,
            });

            // Send initialized notification
            let _notify = JsonRpcRequest::new("notifications/initialized", json!({}));
            let _ = transport.send_notification("notifications/initialized", json!({})).await;

            Ok(())
        } else if let Some(error) = response.error {
            Err(anyhow::anyhow!(
                "Initialize failed: {} (code {})",
                error.message,
                error.code
            ))
        } else {
            Err(anyhow::anyhow!("Initialize failed: no result or error"))
        }
    }

    /// Discover available tools from the MCP server
    pub async fn discover_tools(&self) -> Result<Vec<Tool>> {
        if !self.is_connected().await {
            return Err(anyhow::anyhow!("Client not connected"));
        }

        info!("Discovering MCP tools...");

        let transport = self
            .transport
            .read()
            .await
            .clone()
            .ok_or_else(|| anyhow::anyhow!("Transport not available"))?;

        // Send tools/list request
        let request = JsonRpcRequest::new("tools/list", json!({}))
            .with_id(serde_json::json!(2));

        let response = transport
            .send_request(request)
            .await
            .context("Failed to send tools/list request")?;

        if let Some(result) = response.result {
            let tools_result: ToolsListResult =
                serde_json::from_value(result).context("Failed to parse tools list")?;

            info!("Discovered {} tools", tools_result.tools.len());

            // Store tools
            let mut tools_guard = self.tools.write().await;
            *tools_guard = tools_result.tools.clone();

            Ok(tools_result.tools)
        } else if let Some(error) = response.error {
            Err(anyhow::anyhow!(
                "Tools list failed: {} (code {})",
                error.message,
                error.code
            ))
        } else {
            Err(anyhow::anyhow!("Tools list failed: no result or error"))
        }
    }

    /// Call a tool on the MCP server
    pub async fn call_tool(
        &self,
        name: &str,
        arguments: serde_json::Value,
    ) -> Result<ToolCallResult> {
        if !self.is_connected().await {
            return Err(anyhow::anyhow!("Client not connected"));
        }

        debug!("Calling MCP tool: {}", name);

        let transport = self
            .transport
            .read()
            .await
            .clone()
            .ok_or_else(|| anyhow::anyhow!("Transport not available"))?;

        let request = ToolCallRequest {
            name: name.to_string(),
            arguments,
        };

        let rpc_request = JsonRpcRequest::new("tools/call", serde_json::to_value(&request)?)
            .with_id(serde_json::json!(3));

        let response = transport
            .send_request(rpc_request)
            .await
            .context("Failed to send tools/call request")?;

        if let Some(result) = response.result {
            let call_result: ToolCallResult =
                serde_json::from_value(result).context("Failed to parse tool call result")?;
            Ok(call_result)
        } else if let Some(error) = response.error {
            Err(anyhow::anyhow!(
                "Tool call failed: {} (code {})",
                error.message,
                error.code
            ))
        } else {
            Err(anyhow::anyhow!("Tool call failed: no result or error"))
        }
    }

    /// Get discovered tools
    pub async fn get_tools(&self) -> Vec<Tool> {
        self.tools.read().await.clone()
    }

    /// Get server info
    pub async fn get_server_info(&self) -> Option<ServerInfo> {
        self.server_info.read().await.clone()
    }

    /// Check if a tool exists
    pub async fn has_tool(&self, name: &str) -> bool {
        self.tools
            .read()
            .await
            .iter()
            .any(|t| t.name == name)
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        // Ensure cleanup on drop
        let running = self.running.clone();
        let transport = self.transport.clone();
        tokio::spawn(async move {
            running.store(false, Ordering::SeqCst);
            let mut transport_guard = transport.write().await;
            if let Some(t) = transport_guard.take() {
                let _ = t.disconnect().await;
            }
        });
    }
}

/// Builder for creating MCP clients
pub struct McpClientBuilder {
    config: McpClientConfig,
}

impl McpClientBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            config: McpClientConfig::default(),
        }
    }

    /// Set transport configuration
    pub fn transport(mut self, transport: TransportConfig) -> Self {
        self.config.transport = transport;
        self
    }

    /// Set stdio transport
    pub fn stdio(mut self, command: String, args: Vec<String>, env: Vec<(String, String)>) -> Self {
        self.config.transport = TransportConfig::Stdio {
            command,
            args,
            env,
        };
        self
    }

    /// Set SSE transport
    #[cfg(feature = "http-client")]
    pub fn sse(mut self, url: String, headers: Vec<(String, String)>) -> Self {
        self.config.transport = TransportConfig::Sse { url, headers };
        self
    }

    /// Set connection timeout
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.config.timeout_secs = timeout.as_secs();
        self
    }

    /// Set auto-reconnect
    pub fn auto_reconnect(mut self, enabled: bool) -> Self {
        self.config.auto_reconnect = enabled;
        self
    }

    /// Build the client
    pub fn build(self) -> McpClient {
        McpClient::new(self.config)
    }
}

impl Default for McpClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_client_builder() {
        let client = McpClientBuilder::new()
            .stdio("npx".to_string(), vec![], vec![])
            .timeout(Duration::from_secs(60))
            .auto_reconnect(false)
            .build();

        assert!(!client.running.load(Ordering::SeqCst));
        assert!(!client.initialized.load(Ordering::SeqCst));
    }

    #[test]
    fn test_client_default_config() {
        let config = McpClientConfig::default();
        assert_eq!(config.timeout_secs, 30);
        assert!(config.auto_reconnect);
    }

    #[tokio::test]
    async fn test_client_not_connected() {
        let client = McpClient::default();
        assert!(!client.is_connected().await);

        let tools = client.get_tools().await;
        assert!(tools.is_empty());

        let result = client.discover_tools().await;
        assert!(result.is_err());
    }
}

impl Default for McpClient {
    fn default() -> Self {
        Self::new(McpClientConfig::default())
    }
}
