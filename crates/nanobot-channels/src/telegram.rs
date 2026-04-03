//! Telegram channel connector

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

/// Telegram connector configuration
#[derive(Debug, Clone)]
pub struct TelegramConfig {
    pub bot_token: String,
    pub webhook_url: Option<String>,
    pub polling_interval: u32,
}

impl Default for TelegramConfig {
    fn default() -> Self {
        Self {
            bot_token: String::new(),
            webhook_url: None,
            polling_interval: 2, // seconds
        }
    }
}

/// Telegram bot connector using polling
pub struct TelegramConnector {
    config: RwLock<TelegramConfig>,
    auth_storage: Option<Arc<AuthStorage>>,
    running: RwLock<bool>,
}

impl TelegramConnector {
    /// Create a new Telegram connector
    pub fn new() -> Self {
        Self {
            config: RwLock::new(TelegramConfig::default()),
            auth_storage: None,
            running: RwLock::new(false),
        }
    }

    /// Create with auth storage
    pub fn with_auth(auth_storage: Arc<AuthStorage>) -> Self {
        Self {
            config: RwLock::new(TelegramConfig::default()),
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
            storage.get_token("telegram").await
        } else {
            None
        }
    }

    /// Send a message via Telegram Bot API
    async fn send_telegram_message(
        &self,
        chat_id: &str,
        text: &str,
    ) -> Result<()> {
        let token = self.bot_token()
            .await
            .context("Telegram bot token not configured")?;

        let client = reqwest::Client::new();
        let url = format!(
            "https://api.telegram.org/bot{}/sendMessage",
            token
        );

        let response = client
            .post(&url)
            .json(&json!({
                "chat_id": chat_id,
                "text": text,
                "parse_mode": "Markdown",
            }))
            .send()
            .await
            .context("Failed to send Telegram message")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Telegram API error ({}): {}", status, body);
        }

        debug!("Sent Telegram message to {}", chat_id);
        Ok(())
    }

    /// Get updates from Telegram
    async fn get_updates(
        &self,
        offset: Option<u64>,
    ) -> Result<Vec<serde_json::Value>> {
        let token = self.bot_token()
            .await
            .context("Telegram bot token not configured")?;

        let client = reqwest::Client::new();
        let mut url = format!(
            "https://api.telegram.org/bot{}/getUpdates",
            token
        );

        // Add query parameters
        url.push_str("?timeout=30");
        if let Some(off) = offset {
            url.push_str(&format!("&offset={}", off));
        }

        let response = client
            .get(&url)
            .send()
            .await
            .context("Failed to get Telegram updates")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Telegram API error ({}): {}", status, body);
        }

        let body: serde_json::Value = response.json().await?;

        if let Some(result) = body.get("result").and_then(|v| v.as_array()) {
            Ok(result.clone())
        } else {
            Ok(Vec::new())
        }
    }

    /// Process a Telegram update and convert to InboundMessage
    fn process_update(&self, update: &serde_json::Value) -> Option<InboundMessage> {
        // Extract message from update
        let message = update.get("message")?;

        // Get chat info
        let chat = message.get("chat")?;
        let chat_id = chat.get("id")?.as_i64()?.to_string();

        // Get sender info
        let from = message.get("from")?;
        let sender_id = from.get("id")?.as_i64()?.to_string();
        let sender_name = from
            .get("username")
            .or_else(|| from.get("first_name"))
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown")
            .to_string();

        // Get message text
        let text = message.get("text")?.as_str()?;

        // Create inbound message
        Some(
            InboundMessage::new("telegram", sender_id, chat_id, text)
                .with_metadata("sender_name", json!(sender_name)),
        )
    }

    /// Run the polling loop
    async fn run_polling(&self, bus: MessageBus) {
        info!("Starting Telegram polling loop");

        let mut offset: Option<u64> = None;
        let config = self.config.read().await;
        let polling_interval = config.polling_interval;
        drop(config);

        while *self.running.read().await {
            match self.get_updates(offset).await {
                Ok(updates) => {
                    for update in updates {
                        // Update offset to avoid reprocessing
                        if let Some(update_id) = update.get("update_id").and_then(|v| v.as_u64()) {
                            offset = Some(update_id + 1);
                        }

                        // Process the update
                        if let Some(msg) = self.process_update(&update) {
                            info!("Received Telegram message from {}", msg.chat_id);

                            if let Err(e) = bus.publish_inbound(msg).await {
                                error!("Failed to publish Telegram message to bus: {}", e);
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("Telegram polling error: {}", e);
                    // Wait before retrying
                    tokio::time::sleep(tokio::time::Duration::from_secs(polling_interval as u64)).await;
                }
            }

            // Rate limiting - respect polling interval
            tokio::time::sleep(tokio::time::Duration::from_secs(polling_interval as u64)).await;
        }

        info!("Telegram polling loop stopped");
    }
}

#[async_trait]
impl ChannelConnector for TelegramConnector {
    fn name(&self) -> &str {
        "telegram"
    }

    async fn is_authenticated(&self) -> bool {
        self.bot_token().await.is_some()
    }

    async fn authenticate(&mut self, config: &serde_json::Value) -> Result<()> {
        let token = config
            .get("bot_token")
            .and_then(|v| v.as_str())
            .context("Missing 'bot_token' in Telegram config")?;

        // Validate token by making a test API call
        let client = reqwest::Client::new();
        let url = format!("https://api.telegram.org/bot{}/getMe", token);

        let response = client.get(&url).send().await?;

        if !response.status().is_success() {
            anyhow::bail!("Invalid Telegram bot token");
        }

        // Store token in config
        let mut cfg = self.config.write().await;
        cfg.bot_token = token.to_string();

        // Also store in auth storage if available
        if let Some(storage) = &self.auth_storage {
            use crate::auth::ChannelAuth;
            let auth = ChannelAuth::new(token);
            storage.set_channel("telegram", auth).await?;
        }

        info!("Telegram connector authenticated");
        Ok(())
    }

    async fn start(&self, bus: MessageBus) -> Result<()> {
        {
            let mut running = self.running.write().await;
            if *running {
                warn!("Telegram connector is already running");
                return Ok(());
            }
            *running = true;
        }

        // Check authentication
        if !self.is_authenticated().await {
            anyhow::bail!("Telegram connector not authenticated");
        }

        let config = self.config.read().await;
        let webhook_url = config.webhook_url.clone();
        drop(config);

        // Start polling or webhook mode
        if let Some(webhook) = webhook_url {
            // Webhook mode - not yet implemented
            warn!("Telegram webhook mode not yet implemented, falling back to polling");
            drop(webhook);
        }

        // Run polling loop in background
        let this = self.clone();
        tokio::spawn(async move {
            this.run_polling(bus).await;
        });

        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        let mut running = self.running.write().await;
        *running = false;
        info!("Telegram connector stopping");
        Ok(())
    }

    async fn status(&self) -> ChannelStatus {
        let config = self.config.read().await;
        let running = *self.running.read().await;

        let mut status = ChannelStatus::new("telegram")
            .with_authenticated(!config.bot_token.is_empty())
            .with_running(running);

        if !config.bot_token.is_empty() {
            status = status.with_metadata("configured", json!(true));
        }

        if let Some(webhook) = &config.webhook_url {
            status = status.with_metadata("webhook_url", json!(webhook));
        }

        status
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Default for TelegramConnector {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for TelegramConnector {
    fn clone(&self) -> Self {
        Self {
            config: RwLock::new(self.config.blocking_read().clone()),
            auth_storage: self.auth_storage.clone(),
            running: RwLock::new(*self.running.blocking_read()),
        }
    }
}
