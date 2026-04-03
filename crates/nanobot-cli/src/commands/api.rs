//! API command - Start the OpenAI-compatible API server

use anyhow::{Context, Result};
use nanobot_api::{ApiServer, ApiServerConfig, ApiState};
use nanobot_config::{Config, ConfigLoader};
use nanobot_bus::MessageBus;
use std::path::PathBuf;
use tracing::info;

/// Run the API server command
pub async fn run(
    host: Option<String>,
    port: Option<u16>,
    api_key: Option<String>,
    config_path: Option<&str>,
) -> Result<()> {
    // Load config
    let config_path = config_path
        .map(|p: &str| PathBuf::from(p))
        .or_else(|| nanobot_config::ConfigPaths::default().map(|p| p.config_file))
        .unwrap_or_else(|| {
            dirs::home_dir()
                .map(|d| d.join(".nanobot").join("config.json"))
                .unwrap_or_else(|| ".nanobot/config.json".into())
        });

    let loader = ConfigLoader::new(&config_path);
    let config = loader.load().context("Failed to load config")?;

    // Get API settings from config or command line
    let host = host.unwrap_or_else(|| config.api.host.clone());
    let port = port.unwrap_or(config.api.port);
    let api_key = api_key.or_else(|| std::env::var("RUSTBOT_API_KEY").ok());

    // Create message bus
    let message_bus = MessageBus::new();

    // Create API state
    let state = ApiState {
        config: config.clone(),
        message_bus,
    };

    // Create API server config
    let server_config = ApiServerConfig {
        host,
        port,
        api_key,
    };

    // Create and start server
    let server = ApiServer::new(server_config, state);

    println!("🚀 Starting RustBot API server");
    println!("   Host: {}", server.host());
    println!("   Port: {}", server.port());
    if server.auth().is_enabled().await {
        println!("   Authentication: Enabled");
    } else {
        println!("   Authentication: Disabled (warning: anyone can access)");
    }
    println!();
    println!("Endpoints:");
    println!("   GET  /health              - Health check");
    println!("   GET  /v1/models           - List models");
    println!("   GET  /v1/models/:id       - Get model info");
    println!("   POST /v1/chat/completions - Chat completion");
    println!();

    info!("Starting API server on {}:{}", server.host(), server.port());

    server.run().await.context("API server failed")?;

    Ok(())
}
