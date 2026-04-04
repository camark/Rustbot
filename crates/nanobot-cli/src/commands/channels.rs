//! Channels command - Manage messaging channel integrations

use anyhow::{Context, Result};
use nanobot_channels::{AuthStorage, ChannelManager, ChannelRegistry, create_default_registry};
use nanobot_channels::auth::ChannelAuth;
use nanobot_config::{Config, ConfigLoader};
use std::path::PathBuf;
use tracing::{info, debug, error};
use std::sync::Arc;
use nanobot_bus::MessageBus;
use nanobot_core::{AgentLoop, AgentLoopConfig};
use nanobot_providers::{create_provider_from_spec, match_provider, ProviderBackendType};

/// Run the channels login command
pub async fn login(channel_name: String, force: bool, config_path: Option<&str>) -> Result<()> {
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

    // Get config directory for auth storage
    let config_dir = config_path.parent().unwrap_or_else(|| std::path::Path::new("."));

    // Create registry
    let registry = create_default_registry();

    // Check if channel exists
    if !registry.contains(&channel_name).await {
        anyhow::bail!(
            "Unknown channel '{}'. Available channels: {:?}",
            channel_name,
            registry.list_names().await
        );
    }

    // Check if already authenticated (async)
    let auth_storage = AuthStorage::new(config_dir).await?;
    if !force && auth_storage.is_authenticated(&channel_name).await {
        println!("Channel '{}' is already authenticated.", channel_name);
        println!("Use --force to re-authenticate.");
        return Ok(());
    }

    println!("🔐 Authenticating channel: {}", channel_name);
    println!();

    // Get channel-specific config from user input
    let channel_config = match channel_name.as_str() {
        "telegram" => {
            println!("Enter your Telegram Bot Token (from @BotFather):");
            let token = read_line()?;
            serde_json::json!({
                "bot_token": token.trim(),
            })
        }
        "discord" => {
            println!("Enter your Discord Bot Token:");
            let token = read_line()?;
            let mut config = serde_json::json!({
                "bot_token": token.trim(),
            });
            println!("Enter Guild ID (optional, press Enter to skip):");
            let guild_id = read_line()?;
            if !guild_id.trim().is_empty() {
                config["guild_id"] = serde_json::json!(guild_id.trim());
            }
            config
        }
        "feishu" => {
            println!("Enter your Feishu App ID:");
            let app_id = read_line()?;
            println!("Enter your Feishu App Secret:");
            let app_secret = read_line()?;
            println!("Enter your Feishu Verification Token:");
            let verification_token = read_line()?;
            serde_json::json!({
                "app_id": app_id.trim(),
                "app_secret": app_secret.trim(),
                "verification_token": verification_token.trim(),
            })
        }
        "qq" => {
            println!("Enter your QQ Bot App ID:");
            let app_id = read_line()?;
            println!("Enter your QQ Bot Client Secret:");
            let client_secret = read_line()?;
            println!("Enter your Bot QQ number (optional, press Enter to skip):");
            let bot_qq = read_line()?;
            let mut config = serde_json::json!({
                "app_id": app_id.trim(),
                "client_secret": client_secret.trim(),
            });
            if !bot_qq.trim().is_empty() {
                config["bot_qq"] = serde_json::json!(bot_qq.trim());
            }
            config
        }
        _ => {
            anyhow::bail!("Interactive login not supported for channel '{}'", channel_name);
        }
    };

    // Store credentials in auth storage
    let auth = match channel_name.as_str() {
        "telegram" => {
            let token = channel_config["bot_token"].as_str().unwrap_or("");
            ChannelAuth::new(token)
        }
        "discord" => {
            let token = channel_config["bot_token"].as_str().unwrap_or("");
            let mut auth = ChannelAuth::new(token);
            if let Some(guild_id) = channel_config.get("guild_id").and_then(|v| v.as_str()) {
                auth = auth.with_extra("guild_id", serde_json::json!(guild_id));
            }
            auth
        }
        "feishu" => {
            let app_secret = channel_config["app_secret"].as_str().unwrap_or("");
            ChannelAuth::new(app_secret)
                .with_extra("app_id", channel_config["app_id"].clone())
                .with_extra(
                    "verification_token",
                    channel_config["verification_token"].clone(),
                )
        }
        "qq" => {
            let token = channel_config["client_secret"].as_str().unwrap_or("");
            ChannelAuth::new(token)
                .with_extra("app_id", channel_config["app_id"].clone())
                .with_extra("bot_qq", channel_config["bot_qq"].clone())
        }
        _ => {
            anyhow::bail!("Unsupported channel '{}'", channel_name);
        }
    };

    auth_storage.set_channel(&channel_name, auth).await?;

    println!();
    println!("✅ Channel '{}' authenticated successfully!", channel_name);

    Ok(())
}

/// Run the channels status command
pub async fn status(config_path: Option<&str>) -> Result<()> {
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

    // Get config directory for auth storage
    let config_dir = config_path.parent().unwrap_or_else(|| std::path::Path::new("."));

    // Load auth storage
    let auth_storage = AuthStorage::new(config_dir).await?;

    // Create registry
    let registry = create_default_registry();

    println!("📡 Channel Status");
    println!();

    let channel_names = registry.list_names().await;

    if channel_names.is_empty() {
        println!("No channels registered.");
        return Ok(());
    }

    for name in channel_names {
        let authenticated = auth_storage.is_authenticated(&name).await;
        let status_icon = if authenticated { "✅" } else { "❌" };

        println!("{} {}", status_icon, name);

        if authenticated {
            if let Some(auth) = auth_storage.get_channel(&name).await {
                let token_preview = if auth.token.len() > 8 {
                    format!("{}...", &auth.token[..8])
                } else {
                    "***".to_string()
                };
                println!("   Token: {}", token_preview);

                if let Some(expires) = auth.expires_at {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    let expires_in = expires.saturating_sub(now);
                    let days = expires_in / 86400;
                    let hours = (expires_in % 86400) / 3600;
                    println!("   Expires: {}d {}h", days, hours);
                }
            }
        } else {
            println!("   Not authenticated. Run: rustbot channels login {}", name);
        }
        println!();
    }

    Ok(())
}

/// Run the channels start command
pub async fn start(channel_name: String, config_path: Option<&str>) -> Result<()> {
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

    // Get config directory for auth storage
    let config_dir = config_path.parent().unwrap_or_else(|| std::path::Path::new("."));

    // Load auth storage
    let auth_storage = AuthStorage::new(config_dir).await?;

    // Check authentication
    if !auth_storage.is_authenticated(&channel_name).await {
        anyhow::bail!(
            "Channel '{}' is not authenticated. Run: rustbot channels login {}",
            channel_name,
            channel_name
        );
    }

    // Create registry
    let registry = create_default_registry();

    // Get channel connector
    let connector = registry
        .get(&channel_name)
        .await
        .ok_or_else(|| anyhow::anyhow!("Channel '{}' not found", channel_name))?;

    println!("🚀 Starting channel: {}", channel_name);

    // For Feishu and QQ, we need to configure the connector with auth storage
    // so it can load credentials
    if channel_name == "feishu" {
        // Downcast to FeishuConnector and set auth storage
        if let Some(feishu_connector) = connector.as_any().downcast_ref::<nanobot_channels::feishu::FeishuConnector>() {
            // Configure from auth storage
            if let Some(auth) = auth_storage.get_channel(&channel_name).await {
                feishu_connector.set_config_from_auth(&auth).await?;
            }
        }
    } else if channel_name == "qq" {
        // Downcast to QQConnector and set auth storage
        if let Some(qq_connector) = connector.as_any().downcast_ref::<nanobot_channels::qq::QQConnector>() {
            // Configure from auth storage
            if let Some(auth) = auth_storage.get_channel(&channel_name).await {
                qq_connector.set_config_from_auth(&auth).await?;
            }
        }
    }

    // Create message bus for channel communication
    let message_bus = MessageBus::new();

    info!("Message bus created for channel {}", channel_name);

    // Create provider for AI responses
    let model = config.agents.defaults.model.clone();
    info!("Using model: {} for channel {}", model, channel_name);
    let (provider_config, provider_spec) = match_provider(
        &config.providers,
        Some(&model),
        &config.agents.defaults.provider,
    ).context("No provider configured. Please add your API key to the config.")?;

    let api_key = provider_config.api_key.clone();
    let api_base = provider_config.api_base.clone()
        .or_else(|| provider_spec.default_api_base.map(String::from))
        .unwrap_or_else(|| {
            match provider_spec.backend {
                ProviderBackendType::OpenAiCompat => "https://api.openai.com/v1".to_string(),
                ProviderBackendType::Anthropic => "https://api.anthropic.com".to_string(),
                _ => "https://api.openai.com/v1".to_string(),
            }
        });

    let provider = create_provider_from_spec(
        api_key,
        api_base,
        model.clone(),
        provider_spec,
    );

    // Create agent loop config
    let workspace = config.workspace_path().clone();
    let agent_config = AgentLoopConfig {
        workspace: workspace.clone(),
        model: model.clone(),
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
    .await
    .context("Failed to create agent loop")?;

    let agent_loop = Arc::new(agent_loop);

    // Start the channel connector
    let connector_clone = connector.clone();
    let start_result = connector_clone.start(message_bus.clone()).await;

    match start_result {
        Ok(_) => {
            println!("✅ Channel '{}' started successfully!", channel_name);
            println!("🤖 AI agent ready");
            println!("📡 Listening for messages...");
            println!("Press Ctrl+C to stop");

            // Start agent loop in background
            let agent_loop_clone = agent_loop.clone();
            let channel_name_clone = channel_name.clone();
            tokio::spawn(async move {
                info!("Starting agent loop for channel {}", channel_name_clone);
                if let Err(e) = agent_loop_clone.run().await {
                    error!("Agent loop error: {}", e);
                }
            });

            info!("Agent loop spawned for channel {}", channel_name);

            // Keep the main task alive while the channel runs
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
                debug!("Channel {} is still running...", channel_name);
            }
        }
        Err(e) => {
            anyhow::bail!("Failed to start channel '{}': {}", channel_name, e);
        }
    }
}

/// Run the channels stop command
pub async fn stop(_channel_name: String, _config_path: Option<&str>) -> Result<()> {
    // TODO: Implement stop command
    // This would need to:
    // 1. Find the running channel instance
    // 2. Call connector.stop()
    // 3. Wait for graceful shutdown

    anyhow::bail!("Stop command not yet implemented");
}

/// Helper to read a line from stdin
fn read_line() -> Result<String> {
    use std::io::{self, BufRead};
    let stdin = io::stdin();
    let mut line = String::new();
    stdin.lock().read_line(&mut line)?;
    Ok(line)
}
