//! RustBot CLI
//!
//! Command-line interface for the RustBot AI assistant.

mod commands;
mod stream;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "rustbot")]
#[command(about = "RustBot - AI Assistant Framework", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Config file path
    #[arg(short, long, global = true)]
    config: Option<String>,

    /// Workspace directory
    #[arg(short, long, global = true)]
    workspace: Option<String>,

    /// Enable verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize RustBot configuration
    Onboard {
        /// Interactive setup wizard
        #[arg(long)]
        wizard: bool,
    },

    /// Chat with the agent
    Agent {
        /// Message to send
        #[arg(short, long)]
        message: Option<String>,

        /// Model to use
        #[arg(short, long)]
        model: Option<String>,

        /// Show logs during chat
        #[arg(long)]
        logs: bool,

        /// Disable markdown formatting
        #[arg(long)]
        no_markdown: bool,
    },

    /// Start the gateway server
    Gateway {
        /// Port to listen on
        #[arg(long)]
        port: Option<u16>,
    },

    /// Show status
    Status,

    /// Provider management
    Provider {
        #[command(subcommand)]
        action: ProviderAction,
    },

    /// Channel management
    Channels {
        #[command(subcommand)]
        action: ChannelsAction,
    },

    /// Start the API server
    Api {
        /// Host to listen on
        #[arg(long, default_value = "127.0.0.1")]
        host: Option<String>,

        /// Port to listen on
        #[arg(long)]
        port: Option<u16>,

        /// API key for authentication
        #[arg(long)]
        api_key: Option<String>,
    },

    /// Cron job management
    Cron {
        #[command(subcommand)]
        action: CronAction,
    },

    /// Services management
    Services {
        #[command(subcommand)]
        action: ServicesAction,
    },
}

#[derive(Subcommand)]
enum ProviderAction {
    /// Login to a provider
    Login {
        /// Provider name
        name: String,
    },

    /// List available providers
    List,
}

#[derive(Subcommand)]
enum ChannelsAction {
    /// Login to a channel
    Login {
        /// Channel name
        name: String,

        /// Force re-authentication
        #[arg(long)]
        force: bool,
    },

    /// Show channel status
    Status,
}

#[derive(Subcommand)]
enum CronAction {
    /// Add a new cron job
    Add {
        /// Job name
        name: String,

        /// Cron schedule expression
        schedule: String,
    },

    /// List all cron jobs
    List,

    /// Remove a cron job
    Remove {
        /// Job name
        name: String,
    },

    /// Run a cron job manually
    Run {
        /// Job name
        name: String,
    },
}

#[derive(Subcommand)]
enum ServicesAction {
    /// Show services status
    Status,

    /// Start a service
    Start {
        /// Service name
        name: String,
    },

    /// Stop a service
    Stop {
        /// Service name
        name: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    init_logging(cli.verbose);

    // Run command
    match cli.command {
        Some(Commands::Onboard { wizard }) => {
            commands::onboard::run(wizard, cli.config.as_deref()).await?;
        }
        Some(Commands::Agent { message, model, logs, no_markdown }) => {
            commands::agent::run(message, model, logs, no_markdown, cli.config.as_deref(), cli.workspace.as_deref()).await?;
        }
        Some(Commands::Gateway { port }) => {
            commands::gateway::run(port, cli.config.as_deref()).await?;
        }
        Some(Commands::Status) => {
            commands::status::run(cli.config.as_deref()).await?;
        }
        Some(Commands::Provider { action }) => {
            match action {
                ProviderAction::Login { name } => {
                    eprintln!("Provider login not yet implemented: {}", name);
                }
                ProviderAction::List => {
                    commands::provider::list();
                }
            }
        }
        Some(Commands::Channels { action }) => {
            match action {
                ChannelsAction::Login { name, force } => {
                    commands::channels::login(name, force, cli.config.as_deref()).await?;
                }
                ChannelsAction::Status => {
                    commands::channels::status(cli.config.as_deref()).await?;
                }
            }
        }
        Some(Commands::Api { host, port, api_key }) => {
            commands::api::run(host, port, api_key, cli.config.as_deref()).await?;
        }
        Some(Commands::Cron { action }) => {
            match action {
                CronAction::Add { name, schedule } => {
                    commands::cron::add_job(name, schedule, cli.config.as_deref()).await?;
                }
                CronAction::List => {
                    commands::cron::list_jobs(cli.config.as_deref()).await?;
                }
                CronAction::Remove { name } => {
                    commands::cron::remove_job(name, cli.config.as_deref()).await?;
                }
                CronAction::Run { name } => {
                    commands::cron::run_job(name, cli.config.as_deref()).await?;
                }
            }
        }
        Some(Commands::Services { action }) => {
            match action {
                ServicesAction::Status => {
                    commands::services::status(cli.config.as_deref()).await?;
                }
                ServicesAction::Start { name } => {
                    commands::services::start(name, cli.config.as_deref()).await?;
                }
                ServicesAction::Stop { name } => {
                    commands::services::stop(name, cli.config.as_deref()).await?;
                }
            }
        }
        None => {
            // Default: show help
            Cli::parse_from(["rustbot", "--help"]);
        }
    }

    Ok(())
}

fn init_logging(verbose: bool) {
    use tracing_subscriber::{layer::SubscriberExt, EnvFilter};

    let filter = if verbose {
        "debug"
    } else {
        "info"
    };

    let filter_layer = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(filter));

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(false)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false);

    let subscriber = tracing_subscriber::Registry::default()
        .with(filter_layer)
        .with(fmt_layer);

    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to set global default subscriber");
}
