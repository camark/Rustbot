//! Status command - Show RustBot status

use anyhow::Result;
use nanobot_config::{ConfigLoader, ConfigPaths};
use std::path::PathBuf;

/// Run the status command
pub async fn run(config_path: Option<&str>) -> Result<()> {
    println!("🐈 RustBot Status");
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

    if !config_path.exists() {
        println!("⚠️  Config not found: {}", config_path.display());
        println!();
        println!("Run 'rustbot onboard' to initialize.");
        return Ok(());
    }

    let config = match loader.load() {
        Ok(c) => c,
        Err(e) => {
            println!("❌ Failed to load config: {}", e);
            return Ok(());
        }
    };

    // Configuration status
    println!("Configuration:");
    println!("  Config file: {}", config_path.display());
    println!("  Workspace: {}", config.workspace_path().display());
    println!();

    // Agent defaults
    println!("Agent Defaults:");
    println!("  Model: {}", config.agents.defaults.model);
    println!("  Provider: {}", config.agents.defaults.provider);
    println!("  Max tokens: {}", config.agents.defaults.max_tokens);
    println!("  Temperature: {}", config.agents.defaults.temperature);
    println!();

    // Providers status
    println!("Providers:");

    let providers = [
        ("OpenRouter", &config.providers.openrouter),
        ("Anthropic", &config.providers.anthropic),
        ("OpenAI", &config.providers.openai),
        ("DeepSeek", &config.providers.deepseek),
        ("Azure OpenAI", &config.providers.azure_openai),
        ("Ollama", &config.providers.ollama),
        ("vLLM", &config.providers.vllm),
    ];

    let mut configured_count = 0;
    for (name, provider) in providers {
        let status = if !provider.api_key.is_empty() || provider.api_base.is_some() {
            configured_count += 1;
            "✓"
        } else {
            " "
        };
        println!("  [{}] {}", status, name);
    }

    println!();
    println!("Configured: {}/{}", configured_count, providers.len());

    // Channels status
    println!();
    println!("Channels:");

    let channel_keys = ["telegram", "discord", "feishu", "whatsapp", "slack", "mochat"];
    for channel in &channel_keys {
        let enabled = config.channels.extra.get(*channel)
            .and_then(|v| v.get("enabled"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let status = if enabled { "✓" } else { " " };
        println!("  [{}] {}", status, channel);
    }

    // Tools status
    println!();
    println!("Tools:");
    println!("  Web search: {}", config.tools.web.search.provider);
    println!("  Shell exec: {}", if config.tools.exec.enable { "enabled" } else { "disabled" });
    println!("  Restrict to workspace: {}", config.tools.restrict_to_workspace);

    if !config.tools.mcp_servers.is_empty() {
        println!("  MCP servers: {}", config.tools.mcp_servers.len());
    }

    // Gateway status
    println!();
    println!("Gateway:");
    println!("  Host: {}:{}", config.gateway.host, config.gateway.port);
    println!("  Heartbeat: {}", if config.gateway.heartbeat.enabled { "enabled" } else { "disabled" });

    println!();
    println!("Use 'rustbot agent' to start chatting.");
    println!("Use 'rustbot gateway' to start the gateway server.");

    Ok(())
}
