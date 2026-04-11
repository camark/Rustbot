//! MCP (Model Context Protocol) Client Implementation
//!
//! This module provides MCP client functionality for connecting to external
//! MCP tool servers. It supports both stdio and SSE transports.
//!
//! MCP Specification: https://modelcontextprotocol.io/specification/2025-06-18

pub mod client;
pub mod protocol;
pub mod tools;
pub mod transport;

pub use client::{McpClient, McpClientBuilder, McpClientConfig, ServerInfo};
pub use protocol::{
    ClientCapabilities, ClientInfo, InitializeParams, InitializeResult, JsonRpcRequest,
    JsonRpcResponse, ServerCapabilities, ServerInfo as ProtocolServerInfo, Tool,
    ToolCallContent, ToolCallRequest, ToolCallResult, ToolsCapability, ToolsListResult,
    MCP_PROTOCOL_VERSION, rustbot_client_info,
};
pub use tools::{McpToolIntegration, McpToolRegistry, ToolCallContent as McpToolCallContent, ToolCallResult as McpToolCallResult};
pub use transport::{McpTransport, TransportConfig, create_transport};
