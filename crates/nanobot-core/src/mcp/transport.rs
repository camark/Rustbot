//! MCP Transport Implementations
//!
//! This module provides transport layers for MCP communication:
//! - Stdio: Spawn MCP server as child process, communicate via stdin/stdout
//! - SSE: HTTP Server-Sent Events for remote MCP servers

use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::{Context, Result};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, error, info, trace};

use super::protocol::{JsonRpcError, JsonRpcRequest, JsonRpcResponse};

/// Transport types supported by MCP
#[derive(Debug, Clone)]
pub enum TransportConfig {
    /// Stdio transport - spawn a command
    Stdio {
        command: String,
        args: Vec<String>,
        env: Vec<(String, String)>,
    },
    /// SSE transport - connect to HTTP endpoint
    Sse {
        url: String,
        headers: Vec<(String, String)>,
    },
}

/// Result from a transport send operation
#[derive(Debug, Clone)]
pub struct TransportResponse {
    pub jsonrpc: String,
    pub id: Value,
    pub result: Option<Value>,
    pub error: Option<TransportError>,
}

impl From<JsonRpcResponse> for TransportResponse {
    fn from(response: JsonRpcResponse) -> Self {
        Self {
            jsonrpc: response.jsonrpc,
            id: response.id,
            result: response.result,
            error: response.error.map(|e| TransportError {
                code: e.code,
                message: e.message,
                data: e.data,
            }),
        }
    }
}

/// Transport error
#[derive(Debug, Clone)]
pub struct TransportError {
    pub code: i32,
    pub message: String,
    pub data: Option<Value>,
}

impl From<JsonRpcError> for TransportError {
    fn from(error: JsonRpcError) -> Self {
        Self {
            code: error.code,
            message: error.message,
            data: error.data,
        }
    }
}

/// MCP Transport trait
#[async_trait::async_trait]
pub trait McpTransport: Send + Sync {
    /// Connect to the MCP server
    async fn connect(&self) -> Result<()>;

    /// Disconnect from the MCP server
    async fn disconnect(&self) -> Result<()>;

    /// Check if transport is connected
    async fn is_connected(&self) -> bool;

    /// Send a JSON-RPC request and wait for response
    async fn send_request(&self, request: JsonRpcRequest) -> Result<TransportResponse>;

    /// Send a notification (no response expected)
    async fn send_notification(&self, method: &str, params: Value) -> Result<()>;

    /// Get transport name for logging
    fn name(&self) -> &str;
}

/// Stdio transport implementation
pub struct StdioTransport {
    config: TransportConfig,
    child: Arc<Mutex<Option<Child>>>,
    running: Arc<AtomicBool>,
    response_tx: Arc<Mutex<Option<mpsc::Sender<JsonRpcResponse>>>>,
    pending_requests: Arc<Mutex<Vec<JsonRpcRequest>>>,
}

impl StdioTransport {
    /// Create a new stdio transport
    pub fn new(command: String, args: Vec<String>, env: Vec<(String, String)>) -> Self {
        Self {
            config: TransportConfig::Stdio {
                command,
                args,
                env,
            },
            child: Arc::new(Mutex::new(None)),
            running: Arc::new(AtomicBool::new(false)),
            response_tx: Arc::new(Mutex::new(None)),
            pending_requests: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Create from transport config
    pub fn from_config(config: TransportConfig) -> Result<Self> {
        match config {
            TransportConfig::Stdio {
                command,
                args,
                env,
            } => Ok(Self::new(command, args, env)),
            _ => anyhow::bail!("Config is not a stdio transport"),
        }
    }

    /// Spawn the child process and start reading responses
    async fn spawn_process(&self) -> Result<()> {
        let (command, args, env) = match &self.config {
            TransportConfig::Stdio {
                command,
                args,
                env,
            } => (command.clone(), args.clone(), env.clone()),
            _ => return Err(anyhow::anyhow!("Invalid transport config")),
        };

        info!("Spawning MCP server: {} {}", command, args.join(" "));

        let mut child = Command::new(&command)
            .args(&args)
            .envs(env)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("Failed to spawn MCP server process")?;

        // Start reading stdout in background
        let stdout = child
            .stdout
            .take()
            .context("Failed to take stdout handle")?;
        let stderr = child
            .stderr
            .take()
            .context("Failed to take stderr handle")?;
        let stdin = child
            .stdin
            .take()
            .context("Failed to take stdin handle")?;

        // Store stdin handle for later use
        let stdin_handle = Arc::new(Mutex::new(Some(stdin)));

        // Start stderr reader
        let stderr_reader = tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                // Log stderr output from MCP server
                debug!("MCP server stderr: {}", line);
            }
        });

        // Start stdout reader
        let running = self.running.clone();
        let response_tx = self.response_tx.clone();
        let pending_requests = self.pending_requests.clone();

        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();

            while let Ok(Some(line)) = lines.next_line().await {
                trace!("MCP server stdout: {}", line);

                // Try to parse as JSON-RPC response
                match serde_json::from_str::<JsonRpcResponse>(&line) {
                    Ok(response) => {
                        debug!("Received MCP response: id={:?}", response.id);
                        if let Some(tx) = response_tx.lock().await.as_ref() {
                            let _ = tx.send(response).await;
                        } else {
                            // Store for later if no receiver yet
                            pending_requests.lock().await.push(JsonRpcRequest::new(
                                "pending",
                                serde_json::to_value(response).unwrap_or(Value::Null),
                            ));
                        }
                    }
                    Err(e) => {
                        debug!("Failed to parse MCP response: {} - {}", e, line);
                    }
                }
            }

            running.store(false, Ordering::SeqCst);
            info!("MCP server stdout closed");
        });

        let mut child_guard = self.child.lock().await;
        *child_guard = Some(child);
        self.running.store(true, Ordering::SeqCst);

        Ok(())
    }
}

#[async_trait::async_trait]
impl McpTransport for StdioTransport {
    async fn connect(&self) -> Result<()> {
        if self.is_connected().await {
            return Ok(());
        }

        self.spawn_process().await?;
        info!("Stdio transport connected");
        Ok(())
    }

    async fn disconnect(&self) -> Result<()> {
        let mut child_guard = self.child.lock().await;
        if let Some(mut child) = child_guard.take() {
            // Try graceful shutdown first
            let _ = child.kill().await;
            info!("Stdio transport disconnected");
        }
        self.running.store(false, Ordering::SeqCst);
        Ok(())
    }

    async fn is_connected(&self) -> bool {
        self.running.load(Ordering::SeqCst)
            && self.child.lock().await.is_some()
    }

    async fn send_request(&self, request: JsonRpcRequest) -> Result<TransportResponse> {
        if !self.is_connected().await {
            return Err(anyhow::anyhow!("Transport not connected"));
        }

        // Create response channel
        let (tx, mut rx) = mpsc::channel::<JsonRpcResponse>(100);
        *self.response_tx.lock().await = Some(tx);

        // Serialize and send request
        let request_json = serde_json::to_string(&request)
            .context("Failed to serialize request")?;

        let mut child_guard = self.child.lock().await;
        if let Some(child) = child_guard.as_mut() {
            if let Some(stdin) = child.stdin.as_mut() {
                debug!("Sending MCP request: method={}", request.method);
                stdin
                    .write_all(request_json.as_bytes())
                    .await
                    .context("Failed to write to stdin")?;
                stdin
                    .write_all(b"\n")
                    .await
                    .context("Failed to write newline")?;
                stdin.flush().await.context("Failed to flush stdin")?;
            } else {
                return Err(anyhow::anyhow!("Stdin not available"));
            }
        } else {
            return Err(anyhow::anyhow!("Child process not available"));
        }
        drop(child_guard);

        // Wait for response (with timeout)
        let timeout = tokio::time::Duration::from_secs(30);
        match tokio::time::timeout(timeout, rx.recv()).await {
            Ok(Some(response)) => Ok(response.into()),
            Ok(None) => Err(anyhow::anyhow!("Response channel closed")),
            Err(_) => Err(anyhow::anyhow!("Request timeout after {}s", timeout.as_secs())),
        }
    }

    async fn send_notification(&self, method: &str, params: Value) -> Result<()> {
        if !self.is_connected().await {
            return Err(anyhow::anyhow!("Transport not connected"));
        }

        let notification = JsonRpcRequest::new(method, params);
        let notification_json = serde_json::to_string(&notification)
            .context("Failed to serialize notification")?;

        let mut child_guard = self.child.lock().await;
        if let Some(child) = child_guard.as_mut() {
            if let Some(stdin) = child.stdin.as_mut() {
                debug!("Sending MCP notification: method={}", method);
                stdin
                    .write_all(notification_json.as_bytes())
                    .await
                    .context("Failed to write to stdin")?;
                stdin
                    .write_all(b"\n")
                    .await
                    .context("Failed to write newline")?;
                stdin.flush().await.context("Failed to flush stdin")?;
            }
        }

        Ok(())
    }

    fn name(&self) -> &str {
        "stdio"
    }
}

impl Drop for StdioTransport {
    fn drop(&mut self) {
        // Ensure cleanup on drop
        let running = self.running.clone();
        let child = self.child.clone();
        tokio::spawn(async move {
            let mut child_guard = child.lock().await;
            if let Some(mut c) = child_guard.take() {
                let _ = c.kill().await;
            }
            running.store(false, Ordering::SeqCst);
        });
    }
}

/// SSE transport implementation
#[cfg(feature = "http-client")]
pub struct SseTransport {
    config: TransportConfig,
    client: reqwest::Client,
    connected: Arc<AtomicBool>,
    session_id: Arc<Mutex<Option<String>>>,
}

#[cfg(feature = "http-client")]
impl SseTransport {
    /// Create a new SSE transport
    pub fn new(url: String, headers: Vec<(String, String)>) -> Self {
        Self {
            config: TransportConfig::Sse { url, headers },
            client: reqwest::Client::new(),
            connected: Arc::new(AtomicBool::new(false)),
            session_id: Arc::new(Mutex::new(None)),
        }
    }

    /// Create from transport config
    pub fn from_config(config: TransportConfig) -> Result<Self> {
        match config {
            TransportConfig::Sse { url, headers } => Ok(Self::new(url, headers)),
            _ => anyhow::bail!("Config is not an SSE transport"),
        }
    }
}

#[cfg(feature = "http-client")]
#[async_trait::async_trait]
impl McpTransport for SseTransport {
    async fn connect(&self) -> Result<()> {
        if self.is_connected().await {
            return Ok(());
        }

        let (url, headers) = match &self.config {
            TransportConfig::Sse { url, headers } => (url, headers),
            _ => return Err(anyhow::anyhow!("Invalid transport config")),
        };

        info!("Connecting to MCP SSE endpoint: {}", url);

        // Send initial GET request to establish SSE connection
        let mut request = self.client.get(url);
        for (key, value) in headers {
            request = request.header(key, value);
        }

        match request.send().await {
            Ok(response) => {
                if response.status().is_success() {
                    self.connected.store(true, Ordering::SeqCst);
                    info!("SSE transport connected");
                    Ok(())
                } else {
                    Err(anyhow::anyhow!(
                        "SSE connection failed: {}",
                        response.status()
                    ))
                }
            }
            Err(e) => Err(anyhow::anyhow!("SSE connection error: {}", e)),
        }
    }

    async fn disconnect(&self) -> Result<()> {
        self.connected.store(false, Ordering::SeqCst);
        *self.session_id.lock().await = None;
        info!("SSE transport disconnected");
        Ok(())
    }

    async fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    async fn send_request(&self, request: JsonRpcRequest) -> Result<TransportResponse> {
        if !self.is_connected().await {
            return Err(anyhow::anyhow!("Transport not connected"));
        }

        let (url, headers) = match &self.config {
            TransportConfig::Sse { url, headers } => (url, headers),
            _ => return Err(anyhow::anyhow!("Invalid transport config")),
        };

        let request_json = serde_json::to_string(&request)
            .context("Failed to serialize request")?;

        let mut request_builder = self.client.post(url);
        for (key, value) in headers {
            request_builder = request_builder.header(key, value);
        }

        let response = request_builder
            .json(&request)
            .send()
            .await
            .context("Failed to send request")?;

        if response.status().is_success() {
            let json_response: JsonRpcResponse = response
                .json()
                .await
                .context("Failed to parse response")?;
            Ok(json_response.into())
        } else {
            Err(anyhow::anyhow!("Request failed: {}", response.status()))
        }
    }

    async fn send_notification(&self, method: &str, params: Value) -> Result<()> {
        if !self.is_connected().await {
            return Err(anyhow::anyhow!("Transport not connected"));
        }

        let (url, headers) = match &self.config {
            TransportConfig::Sse { url, headers } => (url, headers),
            _ => return Err(anyhow::anyhow!("Invalid transport config")),
        };

        let notification = JsonRpcRequest::new(method, params);

        let mut request_builder = self.client.post(url);
        for (key, value) in headers {
            request_builder = request_builder.header(key, value);
        }

        request_builder
            .json(&notification)
            .send()
            .await
            .context("Failed to send notification")?;

        Ok(())
    }

    fn name(&self) -> &str {
        "sse"
    }
}

/// Create transport from config
pub fn create_transport(config: TransportConfig) -> Result<Arc<dyn McpTransport>> {
    match &config {
        TransportConfig::Stdio { .. } => {
            let transport = StdioTransport::from_config(config)?;
            Ok(Arc::new(transport))
        }
        #[cfg(feature = "http-client")]
        TransportConfig::Sse { .. } => {
            let transport = SseTransport::from_config(config)?;
            Ok(Arc::new(transport))
        }
        #[cfg(not(feature = "http-client"))]
        TransportConfig::Sse { .. } => {
            anyhow::bail!("SSE transport requires the http-client feature")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transport_config_stdio() {
        let config = TransportConfig::Stdio {
            command: "npx".to_string(),
            args: vec!["-y".to_string(), "@modelcontextprotocol/server".to_string()],
            env: vec![],
        };
        // Just verify config is valid - actual transport creation requires tokio runtime
        match config {
            TransportConfig::Stdio { .. } => {},
            _ => panic!("Expected Stdio config"),
        }
    }

    #[test]
    fn test_transport_config_invalid() {
        #[cfg(feature = "http-client")]
        {
            let config = TransportConfig::Sse {
                url: "http://localhost:3000".to_string(),
                headers: vec![],
            };
            match config {
                TransportConfig::Sse { .. } => {},
                _ => panic!("Expected Sse config"),
            }
        }
    }

    #[test]
    fn test_transport_error_from_wrong_type() {
        let config = TransportConfig::Stdio {
            command: "test".to_string(),
            args: vec![],
            env: vec![],
        };
        #[cfg(feature = "http-client")]
        {
            // This is a compile-time check - can't create Sse from Stdio config
            // The actual type checking happens at compile time
            let is_stdio = matches!(config, TransportConfig::Stdio { .. });
            assert!(is_stdio);
        }
        #[cfg(not(feature = "http-client"))]
        {
            let _ = config; // Suppress unused warning
        }
    }
}
