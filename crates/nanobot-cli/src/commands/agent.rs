//! Agent command - Chat with the AI assistant

use anyhow::{Context, Result};
use nanobot_bus::MessageBus;
use nanobot_config::{Config, ConfigLoader};
use nanobot_core::{AgentLoop, AgentLoopConfig};
use nanobot_providers::{create_provider_from_spec, match_provider, ProviderBackendType};
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};

/// Run the agent command
pub async fn run(
    message: Option<String>,
    model: Option<String>,
    logs: bool,
    _no_markdown: bool,
    config_path: Option<&str>,
    workspace_path: Option<&str>,
) -> Result<()> {
    // Load config
    let config_path = config_path.map(|p: &str| PathBuf::from(p))
        .or_else(|| nanobot_config::ConfigPaths::default().map(|p| p.config_file))
        .unwrap_or_else(|| {
            dirs::home_dir()
                .map(|d| d.join(".nanobot").join("config.json"))
                .unwrap_or_else(|| ".nanobot/config.json".into())
        });

    let loader = ConfigLoader::new(&config_path);
    let config = loader.load().context("Failed to load config. Run 'rustbot onboard' first.")?;

    // Get workspace path
    let workspace = workspace_path
        .map(|p: &str| PathBuf::from(p))
        .unwrap_or_else(|| config.workspace_path().clone());

    // Determine model
    let model = model.unwrap_or_else(|| config.agents.defaults.model.clone());

    // Get provider
    let (provider_config, provider_spec) = match_provider(
        &config.providers,
        Some(&model),
        &config.agents.defaults.provider,
    )
    .context("No provider configured. Please add your API key to the config.")?;

    // Check for OAuth providers (not yet implemented)
    if provider_spec.is_oauth {
        anyhow::bail!(
            "OAuth provider '{}' requires authentication. Run 'rustbot provider login {}' first.",
            provider_spec.name,
            provider_spec.name
        );
    }

    // Get API key and base
    let api_key = provider_config.api_key.clone();
    let api_base = provider_config.api_base.clone()
        .or_else(|| provider_spec.default_api_base.map(String::from))
        .unwrap_or_else(|| {
            // Default based on provider type
            match provider_spec.backend {
                ProviderBackendType::OpenAiCompat => "https://api.openai.com/v1".to_string(),
                ProviderBackendType::Anthropic => "https://api.anthropic.com".to_string(),
                _ => "https://api.openai.com/v1".to_string(),
            }
        });

    // Create provider
    let provider = create_provider_from_spec(
        api_key,
        api_base,
        model.clone(),
        provider_spec,
    );

    // Create message bus
    let bus = MessageBus::new();

    // Create agent loop config
    let agent_config = AgentLoopConfig {
        workspace: workspace.clone(),
        model: model.clone(),
        max_iterations: config.agents.defaults.max_tool_iterations as usize,
        context_window_tokens: config.agents.defaults.context_window_tokens,
        timezone: config.agents.defaults.timezone.clone(),
        tools_config: None,
    };

    // Create agent loop
    let agent_loop = AgentLoop::new(bus, Arc::from(provider), agent_config)
        .await
        .context("Failed to create agent loop")?;

    // Handle message or interactive mode
    if let Some(msg) = message {
        // Single message mode
        run_single_message(&agent_loop, &msg).await?;
    } else {
        // Interactive mode
        run_interactive(&agent_loop, logs).await?;
    }

    Ok(())
}

async fn run_single_message(agent_loop: &AgentLoop, message: &str) -> Result<()> {
    // For single message, we need to publish to bus and wait for response
    println!("Processing: {}", message);

    // Start agent loop in background
    let agent_clone = agent_loop.clone();
    let bus = agent_loop.bus().clone();

    tokio::spawn(async move {
        let _ = agent_clone.run().await;
    });

    // Publish message
    let msg = nanobot_bus::InboundMessage::new("cli", "user", "direct", message);
    bus.publish_inbound(msg).await?;

    // Wait for response with timeout
    match tokio::time::timeout(
        tokio::time::Duration::from_secs(60),
        bus.consume_outbound()
    ).await {
        Ok(Ok(outbound)) => {
            println!("\n{}", outbound.content);
        }
        Ok(Err(_)) => {
            println!("\nError: No response from agent");
        }
        Err(_) => {
            println!("\nError: Request timeout");
        }
    }

    // Stop agent loop
    agent_loop.stop().await;

    Ok(())
}

async fn run_interactive(agent_loop: &AgentLoop, logs: bool) -> Result<()> {
    println!("🐈 RustBot Interactive Mode");
    println!();
    println!("Type your message and press Enter. Commands:");
    println!("  /exit, /quit, exit, quit - Exit the chat");
    println!("  /help - Show help");
    println!();

    if logs {
        println!("[Logs enabled - agent responses will be shown with debug info]");
        println!();
    }

    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin);

    // Start agent loop in background
    let agent_clone = agent_loop.clone();
    tokio::spawn(async move {
        let _ = agent_clone.run().await;
    });

    loop {
        print!("> ");
        let _ = std::io::stdout().flush();

        let mut input = String::new();
        let bytes_read = reader.read_line(&mut input).await?;

        if bytes_read == 0 {
            // EOF (Ctrl+D)
            println!();
            break;
        }

        let input = input.trim();

        if input.is_empty() {
            continue;
        }

        // Check for commands
        match input {
            "/exit" | "/quit" | "exit" | "quit" | ":q" => {
                println!("Goodbye!");
                break;
            }
            "/help" => {
                println!("Commands: /exit, /quit, :q - Exit");
                continue;
            }
            _ => {}
        }

        // Publish message to bus
        let msg = nanobot_bus::InboundMessage::new("cli", "user", "direct", input);
        let _ = agent_loop.bus().publish_inbound(msg).await;

        // Wait for response with timeout
        match tokio::time::timeout(
            tokio::time::Duration::from_secs(120),
            agent_loop.bus().consume_outbound()
        ).await {
            Ok(Ok(outbound)) => {
                println!("\n{}", outbound.content);
            }
            Ok(Err(_)) => {
                println!("Error: No response from agent");
            }
            Err(_) => {
                println!("Error: Request timeout");
            }
        }
    }

    // Stop agent loop
    agent_loop.stop().await;

    Ok(())
}
