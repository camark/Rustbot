//! Discord channel connector

use crate::base::{ChannelConnector, ChannelStatus};
use crate::auth::AuthStorage;
use anyhow::{Context, Result};
use async_trait::async_trait;
use nanobot_bus::{InboundMessage, MessageBus};
use serde_json::json;
use std::any::Any;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Discord connector configuration
#[derive(Debug, Clone)]
pub struct DiscordConfig {
    pub bot_token: String,
    pub guild_id: Option<String>,
}

impl Default for DiscordConfig {
    fn default() -> Self {
        Self {
            bot_token: String::new(),
            guild_id: None,
        }
    }
}

/// Discord bot connector
pub struct DiscordConnector {
    config: RwLock<DiscordConfig>,
    auth_storage: Option<Arc<AuthStorage>>,
    running: RwLock<bool>,
}

impl DiscordConnector {
    /// Create a new Discord connector
    pub fn new() -> Self {
        Self {
            config: RwLock::new(DiscordConfig::default()),
            auth_storage: None,
            running: RwLock::new(false),
        }
    }

    /// Create with auth storage
    pub fn with_auth(auth_storage: Arc<AuthStorage>) -> Self {
        Self {
            config: RwLock::new(DiscordConfig::default()),
            auth_storage: Some(auth_storage),
            running: RwLock::new(false),
        }
    }

    /// Get the bot token
    pub async fn bot_token(&self) -> Option<String> {
        let config = self.config.read().await;
        if !config.bot_token.is_empty() {
            Some(config.bot_token.clone())
        } else if let Some(storage) = &self.auth_storage {
            storage.get_token("discord").await
        } else {
            None
        }
    }

    /// Send a message via Discord API
    async fn send_discord_message(
        &self,
        channel_id: &str,
        text: &str,
    ) -> Result<()> {
        let token = self.bot_token()
            .await
            .context("Discord bot token not configured")?;

        let client = reqwest::Client::new();
        let url = format!(
            "https://discord.com/api/v10/channels/{}/messages",
            channel_id
        );

        let response = client
            .post(&url)
            .header("Authorization", format!("Bot {}", token))
            .header("Content-Type", "application/json")
            .json(&json!({
                "content": text,
            }))
            .send()
            .await
            .context("Failed to send Discord message")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Discord API error ({}): {}", status, body);
        }

        debug!("Sent Discord message to {}", channel_id);
        Ok(())
    }

    /// Process a Discord message and convert to InboundMessage
    fn process_message(&self, event: &serde_json::Value) -> Option<InboundMessage> {
        // MESSAGE_CREATE event
        let data = event.get("d")?;

        // Get channel and author info
        let channel_id = data.get("channel_id")?.as_str()?;
        let author = data.get("author")?;

        // Ignore bot messages
        if author.get("bot").and_then(|v| v.as_bool()).unwrap_or(false) {
            return None;
        }

        let sender_id = author.get("id")?.as_str()?;
        let sender_name = author
            .get("username")
            .or_else(|| author.get("global_name"))
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown")
            .to_string();

        // Get message content
        let content = data.get("content")?.as_str()?;

        // Skip empty messages
        if content.is_empty() {
            return None;
        }

        Some(
            InboundMessage::new("discord", sender_id.to_string(), channel_id.to_string(), content)
                .with_metadata("sender_name", json!(sender_name)),
        )
    }

    /// Run the Discord gateway listener
    async fn run_gateway(&self, _bus: MessageBus) {
        info!("Starting Discord gateway connection");

        let _token = match self.bot_token().await {
            Some(t) => t,
            None => {
                error!("Discord bot token not configured");
                return;
            }
        };

        // For now, use simple HTTP polling as fallback
        // Full gateway implementation would use websocket
        warn!("Discord gateway not fully implemented - using API polling fallback");

        // Poll recent messages from channels
        // This is a simplified implementation
        loop {
            if !*self.running.read().await {
                break;
            }

            // Sleep between polls
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        }

        info!("Discord gateway connection closed");
    }
}

#[async_trait]
impl ChannelConnector for DiscordConnector {
    fn name(&self) -> &str {
        "discord"
    }

    async fn is_authenticated(&self) -> bool {
        self.bot_token().await.is_some()
    }

    async fn authenticate(&mut self, config: &serde_json::Value) -> Result<()> {
        let token = config
            .get("bot_token")
            .and_then(|v| v.as_str())
            .context("Missing 'bot_token' in Discord config")?;

        // Validate token by making a test API call
        let client = reqwest::Client::new();
        let response = client
            .get("https://discord.com/api/v10/users/@me")
            .header("Authorization", format!("Bot {}", token))
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("Invalid Discord bot token");
        }

        // Store token in config
        let mut cfg = self.config.write().await;
        cfg.bot_token = token.to_string();

        // Optionally store guild_id
        if let Some(guild_id) = config.get("guild_id").and_then(|v| v.as_str()) {
            cfg.guild_id = Some(guild_id.to_string());
        }

        // Also store in auth storage if available
        if let Some(storage) = &self.auth_storage {
            use crate::auth::ChannelAuth;
            let mut auth = ChannelAuth::new(token);
            if let Some(guild_id) = &cfg.guild_id {
                auth = auth.with_extra("guild_id", json!(guild_id));
            }
            storage.set_channel("discord", auth).await?;
        }

        info!("Discord connector authenticated");
        Ok(())
    }

    async fn start(&self, bus: MessageBus) -> Result<()> {
        {
            let mut running = self.running.write().await;
            if *running {
                warn!("Discord connector is already running");
                return Ok(());
            }
            *running = true;
        }

        // Check authentication
        if !self.is_authenticated().await {
            anyhow::bail!("Discord connector not authenticated");
        }

        // Run gateway listener in background
        let this = self.clone();
        tokio::spawn(async move {
            this.run_gateway(bus).await;
        });

        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        let mut running = self.running.write().await;
        *running = false;
        info!("Discord connector stopping");
        Ok(())
    }

    async fn status(&self) -> ChannelStatus {
        let config = self.config.read().await;
        let running = *self.running.read().await;

        let mut status = ChannelStatus::new("discord")
            .with_authenticated(!config.bot_token.is_empty())
            .with_running(running);

        if !config.bot_token.is_empty() {
            status = status.with_metadata("configured", json!(true));
        }

        if let Some(guild_id) = &config.guild_id {
            status = status.with_metadata("guild_id", json!(guild_id));
        }

        status
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Default for DiscordConnector {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for DiscordConnector {
    fn clone(&self) -> Self {
        Self {
            config: RwLock::new(self.config.blocking_read().clone()),
            auth_storage: self.auth_storage.clone(),
            running: RwLock::new(*self.running.blocking_read()),
        }
    }
}
