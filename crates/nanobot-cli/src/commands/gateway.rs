//! Gateway command - Start the gateway server

use anyhow::{Context, Result};
use nanobot_config::{ConfigLoader, ConfigPaths};
use std::path::PathBuf;

/// Run the gateway command
pub async fn run(port: Option<u16>, config_path: Option<&str>) -> Result<()> {
    println!("🐈 RustBot Gateway");
    println!();

    // Load config
    let config_path = config_path.map(|p: &str| PathBuf::from(p))
        .or_else(|| ConfigPaths::default().map(|p| p.config_file))
        .unwrap_or_else(|| {
            dirs::home_dir()
                .map(|d| d.join(".nanobot").join("config.json"))
                .unwrap_or_else(|| ".nanobot/config.json".into())
        });

    let loader = ConfigLoader::new(&config_path);
    let config = loader.load().context("Failed to load config. Run 'rustbot onboard' first.")?;

    let port = port.unwrap_or(config.gateway.port);
    let host = &config.gateway.host;

    println!("Starting gateway server...");
    println!("  Host: {}:{}", host, port);
    println!("  Heartbeat: {}", if config.gateway.heartbeat.enabled { "enabled" } else { "disabled" });
    println!();

    // Check for configured channels
    let enabled_channels: Vec<&str> = ["telegram", "discord", "feishu", "whatsapp", "slack", "mochat"]
        .iter()
        .filter(|&&ch| {
            config.channels.extra.get(ch)
                .and_then(|v| v.get("enabled"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
        })
        .cloned()
        .collect();

    if enabled_channels.is_empty() {
        println!("⚠️  No channels enabled. Enable a channel in config.json first.");
        println!();
        println!("Example - Enable Telegram:");
        println!("  \"channels\": {{");
        println!("    \"telegram\": {{");
        println!("      \"enabled\": true,");
        println!("      \"token\": \"YOUR_BOT_TOKEN\"");
        println!("    }}");
        println!("  }}");
        println!();
        return Ok(());
    }

    println!("Enabled channels:");
    for ch in &enabled_channels {
        println!("  - {}", ch);
    }
    println!();

    // TODO: Implement actual gateway server
    println!("[Gateway server implementation - Phase 4]");
    println!("The gateway will:");
    println!("  - Start channel listeners for enabled channels");
    println!("  - Connect to the agent loop");
    println!("  - Route messages between channels and the agent");
    println!();
    println!("Run 'rustbot agent' to use the CLI for now.");

    // Keep running until interrupted
    println!();
    println!("Press Ctrl+C to stop.");

    // Wait for interrupt
    tokio::signal::ctrl_c().await?;
    println!();
    println!("Gateway stopped.");

    Ok(())
}
