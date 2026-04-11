//! MCP Integration Tests

use nanobot_core::mcp::client::{McpClient, McpClientConfig};
use nanobot_core::mcp::tools::McpToolIntegration;
use nanobot_core::mcp::transport::TransportConfig;

#[tokio::test]
#[ignore] // Requires MCP server to be running
async fn test_mcp_filesystem_client() {
    // Create MCP client for filesystem server
    let client = Arc::new(McpClient::new(McpClientConfig {
        transport: TransportConfig::Stdio {
            command: "npx".to_string(),
            args: vec!["-y".to_string(), "@modelcontextprotocol/server-filesystem".to_string(), "/tmp".to_string()],
            env: vec![],
        },
        timeout_secs: 30,
        auto_reconnect: true,
        reconnect_delay_secs: 5,
    }));

    // Connect to server
    let connect_result = client.connect().await;
    assert!(connect_result.is_ok(), "Failed to connect to MCP server: {:?}", connect_result);

    // Discover tools
    let tools = client.discover_tools().await;
    assert!(tools.is_ok(), "Failed to discover tools: {:?}", tools);

    let tools = tools.unwrap();
    assert!(!tools.is_empty(), "No tools discovered");

    println!("Discovered {} tools:", tools.len());
    for tool in &tools {
        println!("  - {}: {}", tool.name, tool.description);
    }

    // Test reading a file
    let read_file_tool = tools.iter().find(|t| t.name == "read_file");
    assert!(read_file_tool.is_some(), "read_file tool not found");

    let client_for_call = client.clone();
    let read_result = client_for_call
        .call_tool("read_file", serde_json::json!({"path": "/tmp/test.txt"}))
        .await;

    // Note: This may fail if file doesn't exist, which is OK for this test
    println!("read_file result: {:?}", read_result);
}

#[tokio::test]
#[ignore] // Requires MCP server to be running
async fn test_mcp_tool_integration() {
    let client = Arc::new(McpClient::new(McpClientConfig::default()));

    if client.connect().await.is_err() {
        println!("MCP server not available, skipping test");
        return;
    }

    let integration = McpToolIntegration::new(client.clone());
    let tools = integration.initialize().await;

    assert!(tools.is_ok(), "Failed to initialize MCP integration");
    let tools = tools.unwrap();

    println!("MCP integration initialized with {} tools", tools.len());
}

use std::sync::Arc;
