//! Feishu (Lark) channel connector

use crate::base::{ChannelConnector, ChannelStatus};
use crate::auth::AuthStorage;
use anyhow::{Context, Result};
use async_trait::async_trait;
use nanobot_bus::{InboundMessage, MessageBus};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Feishu connector configuration
#[derive(Debug, Clone)]
pub struct FeishuConfig {
    pub app_id: String,
    pub app_secret: String,
    pub verification_token: String,
}

impl Default for FeishuConfig {
    fn default() -> Self {
        Self {
            app_id: String::new(),
            app_secret: String::new(),
            verification_token: String::new(),
        }
    }
}

/// Feishu bot connector
pub struct FeishuConnector {
    config: RwLock<FeishuConfig>,
    auth_storage: Option<Arc<AuthStorage>>,
    running: RwLock<bool>,
    tenant_access_token: RwLock<Option<String>>,
}

impl FeishuConnector {
    /// Create a new Feishu connector
    pub fn new() -> Self {
        Self {
            config: RwLock::new(FeishuConfig::default()),
            auth_storage: None,
            running: RwLock::new(false),
            tenant_access_token: RwLock::new(None),
        }
    }

    /// Create with auth storage
    pub fn with_auth(auth_storage: Arc<AuthStorage>) -> Self {
        Self {
            config: RwLock::new(FeishuConfig::default()),
            auth_storage: Some(auth_storage),
            running: RwLock::new(false),
            tenant_access_token: RwLock::new(None),
        }
    }

    /// Get tenant access token
    async fn get_access_token(&self) -> Result<String> {
        // Check cache first
        {
            let token = self.tenant_access_token.read().await;
            if let Some(t) = token.as_ref() {
                return Ok(t.clone());
            }
        }

        // Fetch new token
        let config = self.config.read().await;
        let app_id = config.app_id.clone();
        let app_secret = config.app_secret.clone();
        drop(config);

        if app_id.is_empty() || app_secret.is_empty() {
            anyhow::bail!("Feishu app_id or app_secret not configured");
        }

        let client = reqwest::Client::new();
        let response = client
            .post("https://open.feishu.cn/open-apis/auth/v3/tenant_access_token/internal")
            .header("Content-Type", "application/json")
            .json(&json!({
                "app_id": app_id,
                "app_secret": app_secret,
            }))
            .send()
            .await
            .context("Failed to fetch Feishu access token")?;

        let body: serde_json::Value = response.json().await?;

        if body.get("code").and_then(|v| v.as_i64()) != Some(0) {
            anyhow::bail!("Feishu token fetch failed: {:?}", body);
        }

        let token = body
            .get("tenant_access_token")
            .and_then(|v| v.as_str())
            .context("Missing tenant_access_token in response")?
            .to_string();

        // Cache the token
        *self.tenant_access_token.write().await = Some(token.clone());

        Ok(token)
    }

    /// Send a message via Feishu API
    async fn send_feishu_message(
        &self,
        chat_id: &str,
        text: &str,
    ) -> Result<()> {
        let token = self.get_access_token().await?;

        let client = reqwest::Client::new();
        let url = "https://open.feishu.cn/open-apis/im/v1/messages";

        let response = client
            .post(url)
            .header("Authorization", format!("Bearer {}", token))
            .header("Content-Type", "application/json")
            .json(&json!({
                "receive_id": chat_id,
                "msg_type": "text",
                "content": {
                    "text": text,
                },
            }))
            .send()
            .await
            .context("Failed to send Feishu message")?;

        let body: serde_json::Value = response.json().await?;

        if body.get("code").and_then(|v| v.as_i64()) != Some(0) {
            anyhow::bail!("Feishu send message failed: {:?}", body);
        }

        debug!("Sent Feishu message to {}", chat_id);
        Ok(())
    }

    /// Process a Feishu event and convert to InboundMessage
    fn process_event(&self, event: &serde_json::Value) -> Option<InboundMessage> {
        // Check event type
        let header = event.get("header")?;
        let event_type = header.get("event_type")?.as_str()?;

        // Only handle receive_message events
        if event_type != "im.message.receive_v1" {
            return None;
        }

        let event_data = event.get("event")?;

        // Get message info
        let message = event_data.get("message")?;
        let chat_id = message.get("chat_id")?.as_str()?;
        let sender_id = message.get("sender_id")?.get("open_id")?.as_str()?;
        let content = message.get("content")?;

        // Parse content JSON
        let content_text = content
            .as_object()
            .and_then(|obj| obj.get("text"))
            .or_else(|| content.get("text"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if content_text.is_empty() {
            return None;
        }

        Some(InboundMessage::new(
            "feishu",
            sender_id.to_string(),
            chat_id.to_string(),
            content_text.to_string(),
        ))
    }
}

#[async_trait]
impl ChannelConnector for FeishuConnector {
    fn name(&self) -> &str {
        "feishu"
    }

    async fn is_authenticated(&self) -> bool {
        let config = self.config.read().await;
        !config.app_id.is_empty() && !config.app_secret.is_empty()
    }

    async fn authenticate(&mut self, config: &serde_json::Value) -> Result<()> {
        let app_id = config
            .get("app_id")
            .and_then(|v| v.as_str())
            .context("Missing 'app_id' in Feishu config")?;

        let app_secret = config
            .get("app_secret")
            .and_then(|v| v.as_str())
            .context("Missing 'app_secret' in Feishu config")?;

        let verification_token = config
            .get("verification_token")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // Validate credentials by fetching access token
        let client = reqwest::Client::new();
        let response = client
            .post("https://open.feishu.cn/open-apis/auth/v3/tenant_access_token/internal")
            .header("Content-Type", "application/json")
            .json(&json!({
                "app_id": app_id,
                "app_secret": app_secret,
            }))
            .send()
            .await?;

        let body: serde_json::Value = response.json().await?;

        if body.get("code").and_then(|v| v.as_i64()) != Some(0) {
            anyhow::bail!("Invalid Feishu credentials: {:?}", body);
        }

        // Store config
        let mut cfg = self.config.write().await;
        cfg.app_id = app_id.to_string();
        cfg.app_secret = app_secret.to_string();
        cfg.verification_token = verification_token.to_string();

        // Also store in auth storage if available
        if let Some(storage) = &self.auth_storage {
            use crate::auth::ChannelAuth;
            let auth = ChannelAuth::new(app_secret)
                .with_extra("app_id", json!(app_id))
                .with_extra("verification_token", json!(verification_token));
            storage.set_channel("feishu", auth).await?;
        }

        info!("Feishu connector authenticated");
        Ok(())
    }

    async fn start(&self, _bus: MessageBus) -> Result<()> {
        {
            let mut running = self.running.write().await;
            if *running {
                warn!("Feishu connector is already running");
                return Ok(());
            }
            *running = true;
        }

        // Check authentication
        if !self.is_authenticated().await {
            anyhow::bail!("Feishu connector not authenticated");
        }

        info!("Feishu connector started (webhook mode - requires external webhook handler)");

        // Feishu uses webhooks - the webhook handler would be implemented separately
        // This is a placeholder for the webhook-based approach

        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        let mut running = self.running.write().await;
        *running = false;
        info!("Feishu connector stopping");
        Ok(())
    }

    async fn status(&self) -> ChannelStatus {
        let config = self.config.read().await;
        let running = *self.running.read().await;
        let has_token = self.tenant_access_token.read().await.is_some();

        let mut status = ChannelStatus::new("feishu")
            .with_authenticated(!config.app_id.is_empty() && !config.app_secret.is_empty())
            .with_running(running);

        if !config.app_id.is_empty() {
            status = status.with_metadata("app_id_configured", json!(true));
        }

        if has_token {
            status = status.with_metadata("access_token_cached", json!(true));
        }

        status
    }
}

impl Default for FeishuConnector {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for FeishuConnector {
    fn clone(&self) -> Self {
        Self {
            config: RwLock::new(self.config.blocking_read().clone()),
            auth_storage: self.auth_storage.clone(),
            running: RwLock::new(*self.running.blocking_read()),
            tenant_access_token: RwLock::new(self.tenant_access_token.blocking_read().clone()),
        }
    }
}
