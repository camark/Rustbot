//! API command - Start the OpenAI-compatible API server

use anyhow::{Context, Result};
use nanobot_api::{ApiServer, ApiServerConfig, ApiState};
use nanobot_config::{Config, ConfigLoader};
use nanobot_bus::MessageBus;
use nanobot_core::{AgentLoop, AgentLoopConfig};
use nanobot_providers::{create_provider_from_spec, match_provider, ProviderBackendType};
use std::path::PathBuf;
use std::sync::Arc;
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

    // Get provider config
    let (provider_config, provider_spec) = match_provider(
        &config.providers,
        Some(&config.agents.defaults.model),
        &config.agents.defaults.provider,
    )
    .context("No provider configured. Please add your API key to the config.")?;

    let api_key_from_config = provider_config.api_key.clone();
    let api_base = provider_config.api_base.clone()
        .or_else(|| provider_spec.default_api_base.map(String::from))
        .unwrap_or_else(|| {
            match provider_spec.backend {
                ProviderBackendType::OpenAiCompat => "https://api.openai.com/v1".to_string(),
                ProviderBackendType::Anthropic => "https://api.anthropic.com".to_string(),
                _ => "https://api.openai.com/v1".to_string(),
            }
        });

    // Create provider
    let provider = create_provider_from_spec(
        api_key_from_config,
        api_base,
        config.agents.defaults.model.clone(),
        provider_spec,
    );

    // Create message bus
    let message_bus = MessageBus::new();

    // Create agent loop config
    let agent_config = AgentLoopConfig {
        workspace: config.workspace_path().clone(),
        model: config.agents.defaults.model.clone(),
        max_iterations: config.agents.defaults.max_tool_iterations as usize,
        context_window_tokens: config.agents.defaults.context_window_tokens,
        timezone: config.agents.defaults.timezone.clone(),
        tools_config: None,
    };

    // Create agent loop
    let agent_loop = AgentLoop::new(
        message_bus.clone(),
        Arc::from(provider),
        agent_config,
    )
    .context("Failed to create agent loop")?;

    // Spawn agent loop in background
    let agent_loop_clone = agent_loop.clone();
    tokio::spawn(async move {
        if let Err(e) = agent_loop_clone.run().await {
            tracing::error!("Agent loop error: {}", e);
        }
    });

    info!("Agent loop started in background");

    // Create API state
    let state = ApiState {
        config: config.clone(),
        message_bus,
    };

    // Create API server config
    // Use command-line/env API key for server auth (separate from LLM provider key)
    let server_api_key = api_key.or_else(|| Some("test123".to_string())); // Default test key
    let server_config = ApiServerConfig {
        host,
        port,
        api_key: server_api_key,
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
