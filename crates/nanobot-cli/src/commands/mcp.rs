//! MCP (Model Context Protocol) management commands

use anyhow::{Context, Result};
use nanobot_config::ConfigLoader;
use std::path::PathBuf;

/// List configured MCP servers
pub async fn list(config_path: Option<&str>) -> Result<()> {
    // Load config
    let config_path = config_path
        .map(|p| PathBuf::from(p))
        .or_else(|| nanobot_config::ConfigPaths::default().map(|p| p.config_file))
        .unwrap_or_else(|| {
            dirs::home_dir()
                .map(|d| d.join(".nanobot").join("config.json"))
                .unwrap_or_else(|| ".nanobot/config.json".into())
        });

    let loader = ConfigLoader::new(&config_path);
    let config = loader.load().context("Failed to load config")?;

    let mcp_servers = &config.tools.mcp_servers;

    if mcp_servers.is_empty() {
        println!("No MCP servers configured.");
        println!();
        println!("Add MCP servers to ~/.nanobot/config.json:");
        println!(r#"
{{
  "tools": {{
    "mcpServers": {{
      "filesystem": {{
        "transportType": "stdio",
        "command": "npx",
        "args": ["-y", "@modelcontextprotocol/server-filesystem", "~"]
      }},
      "sqlite": {{
        "transportType": "stdio",
        "command": "npx",
        "args": ["-y", "@modelcontextprotocol/server-sqlite", "/path/to/db.sqlite"]
      }}
    }}
  }}
}}"#);
        return Ok(());
    }

    println!("Configured MCP Servers:");
    println!();

    for (name, server_config) in mcp_servers {
        let transport_type = server_config.transport_type.as_deref().unwrap_or("stdio");

        println!("  ● {}", name);
        println!("    Transport: {}", transport_type);

        if transport_type == "sse" && !server_config.url.is_empty() {
            println!("    URL: {}", server_config.url);
            if !server_config.headers.is_empty() {
                println!("    Headers:");
                for (key, value) in &server_config.headers {
                    println!("      {}: {}", key, value);
                }
            }
        } else {
            println!("    Command: {} {}", server_config.command, server_config.args.join(" "));
            if !server_config.env.is_empty() {
                println!("    Environment:");
                for (key, value) in &server_config.env {
                    println!("      {}: {}", key, value);
                }
            }
        }

        println!("    Timeout: {}s", server_config.tool_timeout);
        println!("    Enabled Tools: {}", server_config.enabled_tools.join(", "));
        println!();
    }

    println!("Total: {} MCP server(s)", mcp_servers.len());
    println!();
    println!("Note: MCP servers are automatically connected when starting 'rustbot agent'.");
    println!("Tools discovered from MCP servers will be available to the LLM.");

    Ok(())
}

/// Show MCP server status
pub async fn status(config_path: Option<&str>) -> Result<()> {
    // Load config
    let config_path = config_path
        .map(|p| PathBuf::from(p))
        .or_else(|| nanobot_config::ConfigPaths::default().map(|p| p.config_file))
        .unwrap_or_else(|| {
            dirs::home_dir()
                .map(|d| d.join(".nanobot").join("config.json"))
                .unwrap_or_else(|| ".nanobot/config.json".into())
        });

    let loader = ConfigLoader::new(&config_path);
    let config = loader.load().context("Failed to load config")?;

    let mcp_servers = &config.tools.mcp_servers;

    if mcp_servers.is_empty() {
        println!("No MCP servers configured.");
        return Ok(());
    }

    println!("MCP Server Status:");
    println!();

    // Note: MCP clients are managed by AgentLoop and only active during runtime
    // This shows configured servers, not runtime status
    for (name, server_config) in mcp_servers {
        let transport_type = server_config.transport_type.as_deref().unwrap_or("stdio");
        let status = "⊙ Configured"; // Static status - runtime status would require AgentLoop integration

        println!("  {} {} ({})", status, name, transport_type);

        if transport_type == "sse" && !server_config.url.is_empty() {
            println!("     URL: {}", server_config.url);
        } else {
            println!("     Command: {} {}", server_config.command, server_config.args.join(" "));
        }
    }

    println!();
    println!("Note: MCP servers are connected when 'rustbot agent' starts.");
    println!("Use 'rustbot agent' to start using MCP tools.");

    Ok(())
}
