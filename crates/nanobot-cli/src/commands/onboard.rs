//! Onboard command - Initialize RustBot configuration

use anyhow::Result;
use nanobot_config::{ConfigLoader, ConfigPaths};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// Run the onboard command
pub async fn run(wizard: bool, config_path: Option<&str>) -> Result<()> {
    let config_path = config_path.map(|p: &str| PathBuf::from(p))
        .or_else(|| ConfigPaths::default().map(|p| p.config_file))
        .unwrap_or_else(|| {
            dirs::home_dir()
                .map(|d| d.join(".nanobot").join("config.json"))
                .unwrap_or_else(|| ".nanobot/config.json".into())
        });

    if wizard {
        run_wizard(&config_path).await
    } else {
        run_simple(&config_path).await
    }
}

/// Run simple onboard - create default config
async fn run_simple(config_path: &Path) -> Result<()> {
    println!("🐈 RustBot - AI Assistant Framework");
    println!();

    // Check if config already exists
    if config_path.exists() {
        println!("Config file already exists: {}", config_path.display());
        println!("Use --wizard to reconfigure, or edit the file directly.");
        return Ok(());
    }

    // Create config directory
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Create default config
    let loader = ConfigLoader::new(config_path);
    let config = loader.create_default()?;

    println!("✨ Created default configuration at: {}", config_path.display());
    println!();
    println!("Next steps:");
    println!("  1. Edit config to add your API key:");
    println!("     {}", config_path.display());
    println!();
    println!("  2. Add your API key (e.g., OpenRouter):");
    println!("     {{");
    println!("       \"providers\": {{");
    println!("         \"openrouter\": {{");
    println!("           \"apiKey\": \"sk-or-v1-xxx\"");
    println!("         }}");
    println!("       }},");
    println!("       \"agents\": {{");
    println!("         \"defaults\": {{");
    println!("           \"model\": \"anthropic/claude-opus-4-5\",");
    println!("           \"provider\": \"openrouter\"");
    println!("         }}");
    println!("       }}");
    println!("     }}");
    println!();
    println!("  3. Start chatting:");
    println!("     rustbot agent -m \"Hello!\"");
    println!();

    // Create workspace directory
    let workspace_dir = config.workspace_path();
    std::fs::create_dir_all(&workspace_dir)?;
    println!("✨ Created workspace at: {}", workspace_dir.display());

    // Create HEARTBEAT.md
    let heartbeat_path = workspace_dir.join("HEARTBEAT.md");
    std::fs::write(&heartbeat_path, "## Periodic Tasks\n\n- [ ] Check weather forecast\n")?;
    println!("✨ Created heartbeat file: {}", heartbeat_path.display());

    Ok(())
}

/// Run interactive setup wizard
async fn run_wizard(config_path: &Path) -> Result<()> {
    println!("🐈 RustBot Setup Wizard");
    println!();
    println!("This will guide you through setting up RustBot.");
    println!();

    // Provider selection
    let provider = select_provider()?;
    println!("Selected provider: {}", provider);

    // API key input
    print!("Enter your {} API key (or press Enter to skip): ", provider);
    let _ = io::stdout().flush();

    let mut api_key = String::new();
    io::stdin().read_line(&mut api_key)?;
    api_key = api_key.trim().to_string();

    // Model selection
    let model = select_model(&provider)?;
    println!("Selected model: {}", model);

    // Create config
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut config = nanobot_config::Config {
        agents: nanobot_config::AgentsConfig {
            defaults: nanobot_config::AgentDefaults {
                model,
                provider: provider.clone(),
                ..Default::default()
            },
        },
        ..Default::default()
    };

    // Set API key based on provider
    let provider_config = match provider.as_str() {
        "openrouter" => &mut config.providers.openrouter,
        "anthropic" => &mut config.providers.anthropic,
        "openai" => &mut config.providers.openai,
        "deepseek" => &mut config.providers.deepseek,
        _ => &mut config.providers.custom,
    };

    if !api_key.is_empty() {
        provider_config.api_key = api_key;
    }

    let loader = ConfigLoader::new(config_path);
    loader.save(&config)?;

    println!();
    println!("✨ Configuration saved to: {}", config_path.display());
    println!();
    println!("You can now start chatting with:");
    println!("  rustbot agent -m \"Hello!\"");
    println!();

    Ok(())
}

fn select_provider() -> Result<String> {
    let providers = vec![
        ("openrouter", "OpenRouter (Recommended - access to all models)"),
        ("anthropic", "Anthropic (Claude direct)"),
        ("openai", "OpenAI (GPT-4, etc.)"),
        ("deepseek", "DeepSeek"),
        ("custom", "Custom (OpenAI-compatible)"),
    ];

    println!("Select a provider:");
    for (i, (name, desc)) in providers.iter().enumerate() {
        println!("  {}. {}", i + 1, desc);
    }
    print!("> ");
    io::stdout().flush().unwrap();

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    let idx: usize = input.trim().parse().unwrap_or(1);
    let provider = providers.get(idx - 1).map(|(n, _)| n).unwrap_or(&"openrouter");

    Ok(provider.to_string())
}

fn select_model(provider: &str) -> Result<String> {
    let models = match provider {
        "anthropic" => vec![
            ("anthropic/claude-opus-4-5", "Claude Opus 4.5 (Most capable)"),
            ("anthropic/claude-sonnet-4-5", "Claude Sonnet 4.5 (Fast & capable)"),
        ],
        "openai" => vec![
            ("openai/gpt-4.1", "GPT-4.1"),
            ("openai/gpt-4o", "GPT-4o"),
            ("openai/o3", "o3"),
        ],
        "openrouter" => vec![
            ("anthropic/claude-opus-4-5", "Claude Opus 4.5"),
            ("openai/gpt-4.1", "GPT-4.1"),
            ("google/gemini-pro-2.0", "Gemini Pro 2.0"),
        ],
        _ => vec![
            ("anthropic/claude-opus-4-5", "Claude Opus 4.5"),
            ("openai/gpt-4.1", "GPT-4.1"),
        ],
    };

    println!();
    println!("Select a model:");
    for (i, (model, desc)) in models.iter().enumerate() {
        println!("  {}. {}", i + 1, desc);
    }
    print!("> ");
    io::stdout().flush().unwrap();

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    let idx: usize = input.trim().parse().unwrap_or(1);
    let model = models.get(idx - 1).map(|(m, _)| m).unwrap_or(&models[0].0);

    Ok(model.to_string())
}
