//! QQ channel connector using QQ Official Bot WebSocket API
//!
//! Uses WebSocket connection for receiving messages via QQ Official Bot API.
//! Reference: https://bot.q.qq.com/wiki/develop/api-v2/
//!
//! Connection flow:
//! 1. Get access_token from https://bots.qq.com/app/getAppAccessToken
//! 2. Get WebSocket gateway URL from https://api.sgroup.qq.com/gateway/bot
//! 3. Connect to WebSocket URL with shard parameters
//! 4. Handle heartbeat and messages

use crate::base::{ChannelConnector, ChannelStatus};
use crate::auth::AuthStorage;
use anyhow::{Context, Result};
use async_trait::async_trait;
use nanobot_bus::{InboundMessage, MessageBus};
use serde::Deserialize;
use serde_json::json;
use std::any::Any;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, error, info, warn};

/// QQ channel connector configuration
#[derive(Debug, Clone)]
pub struct QQConfig {
    /// QQ Bot App ID
    pub app_id: String,
    /// QQ Bot Client Secret
    pub client_secret: String,
    /// Bot QQ number (for display)
    pub bot_qq: String,
}

impl Default for QQConfig {
    fn default() -> Self {
        Self {
            app_id: String::new(),
            client_secret: String::new(),
            bot_qq: String::new(),
        }
    }
}

/// Access token response
#[derive(Debug, Clone, Deserialize)]
struct AccessTokenResponse {
    access_token: String,
    expires_in: String,
}

/// Gateway bot response
#[derive(Debug, Clone, Deserialize)]
struct GatewayBotResponse {
    url: String,
    shards: u32,
    session_start_limit: Option<SessionStartLimit>,
}

#[derive(Debug, Clone, Deserialize)]
struct SessionStartLimit {
    total: u32,
    remaining: u32,
    reset_after: u64,
    max_concurrency: u32,
}

/// QQ bot connector using official WebSocket API
pub struct QQConnector {
    config: RwLock<QQConfig>,
    auth_storage: Option<Arc<AuthStorage>>,
    running: Arc<Mutex<bool>>,
    message_bus: RwLock<Option<MessageBus>>,
    access_token: RwLock<Option<String>>,
    token_expires_at: RwLock<Option<u64>>,
}

impl QQConnector {
    /// Create a new QQ connector
    pub fn new() -> Self {
        Self {
            config: RwLock::new(QQConfig::default()),
            auth_storage: None,
            running: Arc::new(Mutex::new(false)),
            message_bus: RwLock::new(None),
            access_token: RwLock::new(None),
            token_expires_at: RwLock::new(None),
        }
    }

    /// Create with auth storage
    pub fn with_auth(auth_storage: Arc<AuthStorage>) -> Self {
        Self {
            config: RwLock::new(QQConfig::default()),
            auth_storage: Some(auth_storage),
            running: Arc::new(Mutex::new(false)),
            message_bus: RwLock::new(None),
            access_token: RwLock::new(None),
            token_expires_at: RwLock::new(None),
        }
    }

    /// Load config from auth storage if available
    async fn load_config_from_auth(&self) -> Result<()> {
        if let Some(storage) = &self.auth_storage {
            if let Some(auth) = storage.get_channel("qq").await {
                let mut config = self.config.write().await;
                config.app_id = auth.extra.get("app_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                config.client_secret = auth.token.clone();
                if let Some(bot_qq) = auth.extra.get("bot_qq").and_then(|v| v.as_str()) {
                    config.bot_qq = bot_qq.to_string();
                }
                info!("Loaded QQ config from auth storage");
                return Ok(());
            }
        }
        Ok(())
    }

    /// Set config from ChannelAuth
    pub async fn set_config_from_auth(&self, auth: &crate::auth::ChannelAuth) -> Result<()> {
        let mut config = self.config.write().await;
        config.app_id = auth.extra.get("app_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
        config.client_secret = auth.token.clone();
        if let Some(bot_qq) = auth.extra.get("bot_qq").and_then(|v| v.as_str()) {
            config.bot_qq = bot_qq.to_string();
        }
        info!("Set QQ config from auth: app_id={}, bot_qq={}", config.app_id, config.bot_qq);
        Ok(())
    }

    /// Get access token from QQ API
    async fn get_access_token(&self) -> Result<String> {
        // Check if we have a valid token cached
        {
            let token = self.access_token.read().await;
            let expires_at = self.token_expires_at.read().await;
            if let (Some(t), Some(exp)) = (token.as_ref(), expires_at.as_ref()) {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                // Return cached token if still valid for at least 5 minutes
                if exp > &(now + 300) {
                    debug!("Using cached access token");
                    return Ok(t.clone());
                }
            }
        }

        let config = self.config.read().await;
        if config.app_id.is_empty() || config.client_secret.is_empty() {
            anyhow::bail!("QQ app_id or client_secret not configured");
        }

        let client = reqwest::Client::new();
        let response = client
            .post("https://bots.qq.com/app/getAppAccessToken")
            .header("Content-Type", "application/json")
            .json(&json!({
                "appId": config.app_id,
                "clientSecret": config.client_secret,
            }))
            .send()
            .await
            .context("Failed to get access token")?;

        let result: AccessTokenResponse = response.json().await.context("Failed to parse access token response")?;

        if result.access_token.is_empty() {
            anyhow::bail!("Empty access token from QQ API");
        }

        // Calculate expiry time (expires_in is in seconds, usually 7200)
        let expires_in: u64 = result.expires_in.parse().unwrap_or(7200);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let expires_at = now + expires_in;

        info!("Got access token, expires in {} seconds", expires_in);

        // Cache the token
        *self.access_token.write().await = Some(result.access_token.clone());
        *self.token_expires_at.write().await = Some(expires_at);

        Ok(result.access_token)
    }

    /// Get WebSocket gateway URL from QQ API
    async fn get_gateway_url(&self) -> Result<String> {
        let access_token = self.get_access_token().await?;
        let client = reqwest::Client::new();

        let response = client
            .get("https://api.sgroup.qq.com/gateway/bot")
            .header("Authorization", format!("QQBot {}", access_token))
            .send()
            .await
            .context("Failed to get gateway URL")?;

        let result: GatewayBotResponse = response.json().await.context("Failed to parse gateway response")?;

        info!("Got gateway URL: {}", result.url);
        info!("Shards: {}, Session limit: {:?}", result.shards, result.session_start_limit);

        // Build WebSocket URL with shard parameters (shard 0 of 1 for single connection)
        let ws_url = format!("{}?shard=0&shard_count=1", result.url.trim_end_matches('/'));
        Ok(ws_url)
    }

    /// Send a message via QQ API
    async fn send_qq_message(&self, chat_id: &str, text: &str) -> Result<()> {
        let access_token = self.get_access_token().await?;
        let client = reqwest::Client::new();

        // Determine if it's a group or private message
        let (message_type, target_id) = if chat_id.starts_with("group_") {
            ("group", chat_id.strip_prefix("group_").unwrap_or(chat_id))
        } else if chat_id.starts_with("private_") {
            ("private", chat_id.strip_prefix("private_").unwrap_or(chat_id))
        } else {
            // Default to private message
            ("private", chat_id)
        };

        // Build API endpoint
        let api_url = if message_type == "group" {
            format!("https://api.sgroup.qq.com/groups/{}/messages", target_id)
        } else {
            format!("https://api.sgroup.qq.com/users/{}/messages", target_id)
        };

        let response = client
            .post(&api_url)
            .header("Authorization", format!("QQBot {}", access_token))
            .header("Content-Type", "application/json")
            .json(&json!({
                "content": text,
                "msg_type": 0, // 0 = text
            }))
            .send()
            .await
            .context("Failed to send QQ message")?;

        let status = response.status();
        let body = response.text().await.unwrap_or_default();

        if status.is_success() {
            debug!("Sent QQ message to {} via API", chat_id);
            Ok(())
        } else {
            warn!("QQ API response: {} - {}", status, body);
            anyhow::bail!("QQ API error ({}): {}", status, body);
        }
    }

    /// Start WebSocket connection - runs on a dedicated thread with its own runtime
    fn spawn_websocket_task(
        bus: MessageBus,
        config: QQConfig,
        running: Arc<Mutex<bool>>,
        connector: Arc<QQConnector>,
    ) {
        info!(
            "Starting QQ WebSocket connection (app_id: {}, bot_qq: {})",
            config.app_id, config.bot_qq
        );

        // Use std::thread::spawn to create a dedicated thread for the WebSocket connection
        std::thread::spawn(move || {
            // Create a multi-threaded runtime for this thread
            let rt = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(3)
                .thread_name("qq-ws")
                .enable_all()
                .build()
                .expect("Failed to create Tokio runtime");

            rt.block_on(async {
                // Get gateway URL
                let ws_url = match connector.get_gateway_url().await {
                    Ok(url) => url,
                    Err(e) => {
                        error!("Failed to get gateway URL: {}", e);
                        return;
                    }
                };

                info!("Connecting to QQ WebSocket: {}", ws_url);

                // Connect to WebSocket
                let connection_result = tokio_tungstenite::connect_async(&ws_url).await;

                match connection_result {
                    Ok((ws_stream, _)) => {
                        info!("QQ WebSocket connected successfully");

                        let (write, mut read) = ws_stream.split();

                        // Wrap write in Arc<Mutex> for sharing between tasks
                        let write = Arc::new(Mutex::new(write));

                        // Spawn heartbeat task
                        let heartbeat_running = running.clone();
                        let heartbeat_ws_tx = write.clone();
                        tokio::spawn(async move {
                            info!("Starting heartbeat with interval: 45s");

                            loop {
                                tokio::time::sleep(Duration::from_secs(45)).await;

                                // Check if running
                                {
                                    let guard = heartbeat_running.lock().await;
                                    if !*guard {
                                        break;
                                    }
                                }

                                // Send heartbeat (OpCode 1 with sequence number)
                                use futures_util::SinkExt;
                                let heartbeat = json!({
                                    "op": 1,
                                    "d": null,
                                });

                                let ws_msg = match serde_json::to_string(&heartbeat) {
                                    Ok(s) => s,
                                    Err(e) => {
                                        error!("Failed to serialize heartbeat: {}", e);
                                        continue;
                                    }
                                };

                                let mut tx = heartbeat_ws_tx.lock().await;
                                if let Err(e) = tx.send(tokio_tungstenite::tungstenite::Message::Text(ws_msg.into())).await {
                                    error!("Failed to send heartbeat: {}", e);
                                    break;
                                }
                                debug!("Heartbeat sent");
                            }
                        });

                        // Clone for outbound handler
                        let outbound_bus = bus.clone();
                        let outbound_running = running.clone();
                        let _outbound_ws_tx = write.clone();

                        // Spawn outbound message handler
                        tokio::spawn(async move {
                            info!("QQ outbound handler started");
                            loop {
                                // Check if we should stop
                                {
                                    let guard = outbound_running.lock().await;
                                    if !*guard {
                                        info!("QQ outbound handler stopping");
                                        break;
                                    }
                                }

                                // Try to get outbound message
                                match outbound_bus.try_consume_outbound().await {
                                    Some(outbound) => {
                                        info!(
                                            "QQ outbound: received message for chat_id={}, content_len={}",
                                            outbound.chat_id, outbound.content.len()
                                        );

                                        // Send message via API (WebSocket doesn't support sending directly)
                                        match connector.send_qq_message(&outbound.chat_id, &outbound.content).await {
                                            Ok(_) => {
                                                info!("QQ outbound: message sent to {}", outbound.chat_id);
                                            }
                                            Err(e) => {
                                                error!("QQ outbound: failed to send message: {}", e);
                                            }
                                        }
                                    }
                                    None => {
                                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                                    }
                                }
                            }
                        });

                        // Wait a bit for outbound handler to start
                        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

                        // Read messages from WebSocket
                        use futures_util::StreamExt;
                        loop {
                            // Check if we should stop
                            {
                                let guard = running.lock().await;
                                if !*guard {
                                    info!("QQ WebSocket stopping");
                                    break;
                                }
                            }

                            match read.next().await {
                                Some(Ok(msg)) => {
                                    if let tokio_tungstenite::tungstenite::Message::Text(text) = msg {
                                        // Parse WebSocket message
                                        if let Ok(event) = serde_json::from_str::<serde_json::Value>(&text) {
                                            let op = event.get("op").and_then(|v| v.as_u64());
                                            let t = event.get("t").and_then(|v| v.as_str());
                                            let d = event.get("d");

                                            // Handle different opcodes
                                            match op {
                                                Some(0) => {
                                                    // Dispatch event
                                                    match t {
                                                        Some("C2C_MESSAGE_CREATE") | Some("DIRECT_MESSAGE_CREATE") => {
                                                            // Private message
                                                            let default_author = json!({});
                                                            let author = d.and_then(|d| d.get("author"))
                                                                .unwrap_or(&default_author);
                                                            let sender_id = author.get("id")
                                                                .and_then(|v| v.as_str())
                                                                .unwrap_or("");
                                                            let content = d.and_then(|d| d.get("content"))
                                                                .and_then(|v| v.as_str())
                                                                .unwrap_or("");

                                                            info!("QQ private message from {}: {}", sender_id, content);

                                                            let chat_id = format!("private_{}", sender_id);
                                                            let inbound = InboundMessage::new(
                                                                "qq",
                                                                sender_id.to_string(),
                                                                chat_id.clone(),
                                                                content.to_string(),
                                                            );

                                                            let bus_clone = bus.clone();
                                                            tokio::spawn(async move {
                                                                match bus_clone.publish_inbound(inbound).await {
                                                                    Ok(_) => info!("QQ: published inbound message"),
                                                                    Err(e) => error!("QQ: failed to publish inbound: {}", e),
                                                                }
                                                            });
                                                        }
                                                        Some("GROUP_AT_MESSAGE_CREATE") | Some("GROUP_MESSAGE_CREATE") => {
                                                            // Group message
                                                            let group_id = d.and_then(|d| d.get("group_id"))
                                                                .and_then(|v| v.as_str())
                                                                .unwrap_or("");
                                                            let default_author = json!({});
                                                            let author = d.and_then(|d| d.get("author"))
                                                                .unwrap_or(&default_author);
                                                            let sender_id = author.get("id")
                                                                .and_then(|v| v.as_str())
                                                                .unwrap_or("");
                                                            let content = d.and_then(|d| d.get("content"))
                                                                .and_then(|v| v.as_str())
                                                                .unwrap_or("");

                                                            info!("QQ group message from {} in {}: {}", sender_id, group_id, content);

                                                            let chat_id = format!("group_{}", group_id);
                                                            let inbound = InboundMessage::new(
                                                                "qq",
                                                                sender_id.to_string(),
                                                                chat_id.clone(),
                                                                content.to_string(),
                                                            );

                                                            let bus_clone = bus.clone();
                                                            tokio::spawn(async move {
                                                                match bus_clone.publish_inbound(inbound).await {
                                                                    Ok(_) => info!("QQ: published inbound message"),
                                                                    Err(e) => error!("QQ: failed to publish inbound: {}", e),
                                                                }
                                                            });
                                                        }
                                                        _ => {
                                                            debug!("QQ: unknown dispatch event type: {:?}", t);
                                                        }
                                                    }
                                                }
                                                Some(1) => {
                                                    // Heartbeat ACK - server acknowledged our heartbeat
                                                    debug!("QQ: heartbeat acknowledged");
                                                }
                                                Some(10) => {
                                                    // Hello - contains heartbeat interval
                                                    if let Some(d_obj) = d.and_then(|v| v.as_object()) {
                                                        if let Some(interval_ms) = d_obj.get("heartbeat_interval").and_then(|v| v.as_u64()) {
                                                            info!("QQ: Hello received, heartbeat interval: {}ms", interval_ms);
                                                        }
                                                    }
                                                }
                                                _ => {
                                                    debug!("QQ: unknown opcode: {:?}", op);
                                                }
                                            }
                                        }
                                    } else if let tokio_tungstenite::tungstenite::Message::Ping(data) = msg {
                                        // Auto pong
                                        use futures_util::SinkExt;
                                        let mut write_guard = write.lock().await;
                                        let _ = write_guard.send(tokio_tungstenite::tungstenite::Message::Pong(data)).await;
                                    } else if let tokio_tungstenite::tungstenite::Message::Close(_) = msg {
                                        info!("QQ WebSocket received close frame");
                                        break;
                                    }
                                }
                                Some(Err(e)) => {
                                    error!("QQ WebSocket error: {}", e);
                                    break;
                                }
                                None => {
                                    info!("QQ WebSocket stream ended");
                                    break;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!("QQ WebSocket connection failed: {:?}", e);

                        // Keep running until stopped even if WebSocket failed
                        loop {
                            tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
                            let running_guard = running.lock().await;
                            if !*running_guard {
                                info!("QQ WebSocket stopping after error");
                                break;
                            }
                        }
                    }
                }

                info!("QQ WebSocket task ended");
            });
        });
    }
}

#[async_trait]
impl ChannelConnector for QQConnector {
    fn name(&self) -> &str {
        "qq"
    }

    async fn is_authenticated(&self) -> bool {
        let config = self.config.read().await;
        !config.app_id.is_empty() && !config.client_secret.is_empty()
    }

    async fn authenticate(&mut self, config: &serde_json::Value) -> Result<()> {
        let app_id = config
            .get("app_id")
            .and_then(|v| v.as_str())
            .context("Missing 'app_id' in QQ config")?;

        let client_secret = config
            .get("client_secret")
            .and_then(|v| v.as_str())
            .context("Missing 'client_secret' in QQ config")?;

        let bot_qq = config
            .get("bot_qq")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // Validate credentials by attempting to get access token
        let client = reqwest::Client::new();
        let response = client
            .post("https://bots.qq.com/app/getAppAccessToken")
            .header("Content-Type", "application/json")
            .json(&json!({
                "appId": app_id,
                "clientSecret": client_secret,
            }))
            .send()
            .await
            .context("Failed to validate QQ credentials")?;

        let result: AccessTokenResponse = response.json().await.context("Invalid credentials")?;

        if result.access_token.is_empty() {
            anyhow::bail!("Invalid QQ credentials: empty access token");
        }

        // Store config
        let mut cfg = self.config.write().await;
        cfg.app_id = app_id.to_string();
        cfg.client_secret = client_secret.to_string();
        cfg.bot_qq = bot_qq.to_string();

        // Also store in auth storage if available
        if let Some(storage) = &self.auth_storage {
            use crate::auth::ChannelAuth;
            let auth = ChannelAuth::new(client_secret)
                .with_extra("app_id", json!(app_id))
                .with_extra("bot_qq", json!(bot_qq));
            storage.set_channel("qq", auth).await?;
        }

        info!("QQ connector authenticated: app_id={}, bot_qq={}", app_id, bot_qq);
        Ok(())
    }

    async fn start(&self, bus: MessageBus) -> Result<()> {
        {
            let mut running = self.running.lock().await;
            if *running {
                warn!("QQ connector is already running");
                return Ok(());
            }
            *running = true;
        }

        // Load config from auth storage if available
        self.load_config_from_auth().await?;

        // Check authentication
        if !self.is_authenticated().await {
            anyhow::bail!("QQ connector not authenticated");
        }

        // Store message bus for potential future use
        *self.message_bus.write().await = Some(bus.clone());

        // Get config snapshot for spawning task
        let config = self.config.read().await.clone();

        // Spawn WebSocket task
        Self::spawn_websocket_task(bus, config, self.running.clone(), Arc::new(self.clone()));

        info!("QQ connector started (Official WebSocket mode)");
        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        let mut running = self.running.lock().await;
        *running = false;
        info!("QQ connector stopping");
        Ok(())
    }

    async fn status(&self) -> ChannelStatus {
        let config = self.config.read().await;
        let running = *self.running.lock().await;
        let has_token = self.access_token.read().await.is_some();

        let mut status = ChannelStatus::new("qq")
            .with_authenticated(!config.app_id.is_empty() && !config.client_secret.is_empty())
            .with_running(running);

        if !config.app_id.is_empty() {
            status = status.with_metadata("app_id_configured", json!(true));
        }

        if !config.bot_qq.is_empty() {
            status = status.with_metadata("bot_qq", json!(config.bot_qq.clone()));
        }

        if has_token {
            status = status.with_metadata("access_token_cached", json!(true));
        }

        status
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Default for QQConnector {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for QQConnector {
    fn clone(&self) -> Self {
        let config = self.config.try_read()
            .map(|c| c.clone())
            .unwrap_or(QQConfig::default());
        let message_bus = self.message_bus.try_read()
            .map(|m| m.clone())
            .unwrap_or(None);
        let access_token = self.access_token.try_read()
            .map(|t| t.clone())
            .unwrap_or(None);
        let token_expires_at = self.token_expires_at.try_read()
            .map(|t| *t)
            .unwrap_or(None);

        Self {
            config: RwLock::new(config),
            auth_storage: self.auth_storage.clone(),
            running: self.running.clone(),
            message_bus: RwLock::new(message_bus),
            access_token: RwLock::new(access_token),
            token_expires_at: RwLock::new(token_expires_at),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::ChannelAuth;

    #[test]
    fn test_qq_connector_creation() {
        let connector = QQConnector::new();
        assert_eq!(connector.name(), "qq");
    }

    #[tokio::test]
    async fn test_qq_config_default() {
        let connector = QQConnector::new();
        let status = connector.status().await;
        assert_eq!(status.name, "qq");
        assert!(!status.authenticated);
        assert!(!status.running);
    }

    #[tokio::test]
    async fn test_qq_is_authenticated() {
        let connector = QQConnector::new();

        // Not authenticated initially
        assert!(!connector.is_authenticated().await);

        // Set config
        connector
            .set_config_from_auth(
                &ChannelAuth::new("test_secret")
                    .with_extra("app_id", json!("123456"))
                    .with_extra("bot_qq", json!("789012")),
            )
            .await
            .unwrap();

        // Should be authenticated after config
        assert!(connector.is_authenticated().await);
    }

    #[tokio::test]
    async fn test_qq_status_metadata() {
        let connector = QQConnector::new();

        // Set config
        connector
            .set_config_from_auth(
                &ChannelAuth::new("test_secret")
                    .with_extra("app_id", json!("123456"))
                    .with_extra("bot_qq", json!("789012")),
            )
            .await
            .unwrap();

        let status = connector.status().await;
        assert!(status.metadata.get("app_id_configured").is_some());
        assert!(status.metadata.get("bot_qq").is_some());
    }
}
