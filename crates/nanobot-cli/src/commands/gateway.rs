//! Gateway command - Start the gateway server

use anyhow::{Context, Result};
use nanobot_channels::{ChannelManager, ChannelRegistry, create_default_registry, AuthStorage};
use nanobot_channels::auth::ChannelAuth;
use nanobot_config::{ConfigLoader, ConfigPaths};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, error};

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
    let mut enabled_channels: Vec<&str> = Vec::new();

    // Check strongly-typed channel configs (telegram, discord, feishu)
    if !config.channels.telegram.bot_token.is_empty() {
        enabled_channels.push("telegram");
    }
    if !config.channels.discord.bot_token.is_empty() {
        enabled_channels.push("discord");
    }
    if !config.channels.feishu.app_id.is_empty() && !config.channels.feishu.app_secret.is_empty() {
        enabled_channels.push("feishu");
    }

    // Also check extra configs for other channels
    let extra_channels = ["whatsapp", "slack", "mochat"]
        .iter()
        .filter(|&&ch| {
            config.channels.extra.get(ch)
                .and_then(|v| v.get("enabled"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
        })
        .cloned();

    enabled_channels.extend(extra_channels);

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

    // Create message bus
    let bus = nanobot_bus::MessageBus::new();

    // Create channel registry
    let registry = create_default_registry();

    // Create auth storage
    let auth_dir = config_path.parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));

    // Create channel manager first (it creates its own auth_storage)
    let mut manager = ChannelManager::new(registry, &auth_dir).await?;

    // Set message bus for channel communication
    manager.set_message_bus(bus.clone());

    // Get manager's auth storage
    let auth_storage = manager.auth_storage().clone();

    // Auto-configure auth from config file for enabled channels
    for channel_name in &enabled_channels {
        match *channel_name {
            "telegram" => {
                if !config.channels.telegram.bot_token.is_empty() {
                    let auth = ChannelAuth::new(&config.channels.telegram.bot_token);
                    let _ = auth_storage.set_channel("telegram", auth).await;
                }
            }
            "discord" => {
                if !config.channels.discord.bot_token.is_empty() {
                    let mut auth = ChannelAuth::new(&config.channels.discord.bot_token);
                    if let Some(guild_id) = &config.channels.discord.guild_id {
                        auth = auth.with_extra("guild_id", serde_json::json!(guild_id));
                    }
                    let _ = auth_storage.set_channel("discord", auth).await;
                }
            }
            "feishu" => {
                if !config.channels.feishu.app_id.is_empty() && !config.channels.feishu.app_secret.is_empty() {
                    let auth = ChannelAuth::new(&config.channels.feishu.app_secret)
                        .with_extra("app_id", serde_json::json!(config.channels.feishu.app_id))
                        .with_extra("verification_token", serde_json::json!(config.channels.feishu.verification_token));
                    let _ = auth_storage.set_channel("feishu", auth).await;
                }
            }
            _ => {}
        }
    }
    for channel_name in &enabled_channels {
        println!("Starting channel: {}", channel_name);
        if let Err(e) = manager.start(channel_name).await {
            error!("Failed to start channel '{}': {}", channel_name, e);
            println!("  ⚠️  Failed to start: {}", e);
        } else {
            println!("  ✓ Started");
        }
    }
    println!();

    // Keep running until interrupted
    println!("Gateway is running. Press Ctrl+C to stop.");
    println!();

    // Wait for interrupt
    tokio::signal::ctrl_c().await?;
    println!();
    println!("Stopping gateway...");

    // Stop all channels
    manager.stop_all().await;

    println!("Gateway stopped.");

    Ok(())
}
