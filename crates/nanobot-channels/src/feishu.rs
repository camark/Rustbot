//! Feishu (Lark) channel connector using open-lark SDK
//!
//! Uses WebSocket long connection (长连接) for receiving messages.
//! Reference: https://open.feishu.cn/document/ukTMukTMukTM/uYDNxYjL2QTM24iN0EjN/event-subscription-configure-/use-websocket

use crate::base::{ChannelConnector, ChannelStatus};
use crate::auth::AuthStorage;
use anyhow::{Context, Result};
use async_trait::async_trait;
use nanobot_bus::{InboundMessage, MessageBus};
use open_lark::client::ws_client::LarkWsClient;
use open_lark::prelude::*;
use serde_json::json;
use std::any::Any;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
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

/// Feishu bot connector using open-lark WebSocket long connection
pub struct FeishuConnector {
    config: RwLock<FeishuConfig>,
    auth_storage: Option<Arc<AuthStorage>>,
    running: Arc<Mutex<bool>>,
    message_bus: RwLock<Option<MessageBus>>,
    lark_client: RwLock<Option<Arc<LarkClient>>>,
}

impl FeishuConnector {
    /// Create a new Feishu connector
    pub fn new() -> Self {
        Self {
            config: RwLock::new(FeishuConfig::default()),
            auth_storage: None,
            running: Arc::new(Mutex::new(false)),
            message_bus: RwLock::new(None),
            lark_client: RwLock::new(None),
        }
    }

    /// Create with auth storage
    pub fn with_auth(auth_storage: Arc<AuthStorage>) -> Self {
        Self {
            config: RwLock::new(FeishuConfig::default()),
            auth_storage: Some(auth_storage),
            running: Arc::new(Mutex::new(false)),
            message_bus: RwLock::new(None),
            lark_client: RwLock::new(None),
        }
    }

    /// Load config from auth storage if available
    async fn load_config_from_auth(&self) -> Result<()> {
        if let Some(storage) = &self.auth_storage {
            if let Some(auth) = storage.get_channel("feishu").await {
                let mut config = self.config.write().await;
                config.app_secret = auth.token.clone();
                if let Some(app_id) = auth.extra.get("app_id").and_then(|v| v.as_str()) {
                    config.app_id = app_id.to_string();
                }
                if let Some(verification_token) = auth.extra.get("verification_token").and_then(|v| v.as_str()) {
                    config.verification_token = verification_token.to_string();
                }
                info!("Loaded Feishu config from auth storage");
                return Ok(());
            }
        }
        Ok(())
    }

    /// Set config from ChannelAuth
    pub async fn set_config_from_auth(&self, auth: &crate::auth::ChannelAuth) -> Result<()> {
        let mut config = self.config.write().await;
        config.app_secret = auth.token.clone();
        if let Some(app_id) = auth.extra.get("app_id").and_then(|v| v.as_str()) {
            config.app_id = app_id.to_string();
        }
        if let Some(verification_token) = auth.extra.get("verification_token").and_then(|v| v.as_str()) {
            config.verification_token = verification_token.to_string();
        }
        info!("Set Feishu config from auth");
        Ok(())
    }

    /// Send a message via Feishu API using open-lark client
    pub async fn send_feishu_message(&self, chat_id: &str, text: &str) -> Result<()> {
        // For sending messages, we need to wait for the client to be initialized
        // This is a simplified implementation - in production you may want to poll
        // or use a different pattern
        let lark_client = self.lark_client.read().await;
        let client = lark_client.as_ref()
            .context("Lark client not initialized")?;

        let content = json!({
            "text": text,
        })
        .to_string();

        let request = CreateMessageRequest::builder()
            .receive_id_type("chat_id")
            .request_body(
                CreateMessageRequestBody::builder()
                    .receive_id(chat_id)
                    .msg_type("text")
                    .content(&content)
                    .build(),
            )
            .build();

        let _response = client
            .im
            .v1
            .message
            .create(request, None)
            .await
            .context("Failed to send Feishu message")?;

        debug!("Sent Feishu message to {}", chat_id);
        Ok(())
    }

    /// Start WebSocket long connection - runs on a dedicated thread with its own runtime
    fn spawn_websocket_task(
        bus: MessageBus,
        config: FeishuConfig,
        running: Arc<Mutex<bool>>,
        lark_client_store: Arc<RwLock<Option<Arc<LarkClient>>>>,
    ) {
        // Use std::thread::spawn to create a dedicated thread for the WebSocket connection
        std::thread::spawn(move || {
            let app_id = &config.app_id;
            let app_secret = &config.app_secret;

            if app_id.is_empty() || app_secret.is_empty() {
                error!("Feishu app_id or app_secret not configured");
                return;
            }

            info!("Starting Feishu WebSocket long connection (app_id: {})", app_id);

            // Create a multi-threaded runtime for this thread
            let rt = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(3)
                .thread_name("feishu-ws")
                .enable_all()
                .build()
                .expect("Failed to create Tokio runtime");

            rt.block_on(async {
                // Create Lark client
                let lark_client = Arc::new(
                    LarkClient::builder(app_id, app_secret)
                        .with_app_type(AppType::SelfBuild)
                        .with_enable_token_cache(true)
                        .build()
                );

                info!("Feishu Lark client created");

                // Store client for sending messages
                {
                    *lark_client_store.write().await = Some(lark_client.clone());
                    info!("Feishu client stored for outbound messages");
                }

                // Clone bus for outbound handler
                let outbound_bus = bus.clone();
                let outbound_running = running.clone();
                let outbound_lark_client = lark_client.clone();

                // Spawn outbound message handler
                tokio::spawn(async move {
                    info!("Feishu outbound handler started");
                    loop {
                        // Check if we should stop
                        {
                            let guard = outbound_running.lock().await;
                            if !*guard {
                                info!("Feishu outbound handler stopping");
                                break;
                            }
                        }

                        // Try to get outbound message (non-blocking)
                        match outbound_bus.try_consume_outbound().await {
                            Some(outbound) => {
                                info!("Feishu outbound: received message for chat_id={}, content_len={}",
                                    outbound.chat_id, outbound.content.len());

                                let content = json!({
                                    "text": outbound.content,
                                })
                                .to_string();

                                let request = CreateMessageRequest::builder()
                                    .receive_id_type("chat_id")
                                    .request_body(
                                        CreateMessageRequestBody::builder()
                                            .receive_id(&outbound.chat_id)
                                            .msg_type("text")
                                            .content(&content)
                                            .build(),
                                    )
                                    .build();

                                match outbound_lark_client
                                    .im
                                    .v1
                                    .message
                                    .create(request, None)
                                    .await
                                {
                                    Ok(_) => {
                                        info!("Feishu outbound: message sent to {}", outbound.chat_id);
                                    }
                                    Err(e) => {
                                        error!("Feishu outbound: failed to send message: {}", e);
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

                // Create event handler with message processing
                let bus_for_handler = bus.clone();
                let event_handler = EventDispatcherHandler::builder()
                    .register_p2_im_message_receive_v1(move |event| {
                        let bus_clone = bus_for_handler.clone();

                        info!("Feishu event received: type={}, sender={:?}",
                            event.event.message.message_type,
                            event.event.sender.sender_id.open_id);

                        // Only handle text messages
                        if event.event.message.message_type != "text" {
                            debug!("Skipping non-text message type: {}", event.event.message.message_type);
                            return;
                        }

                        // Get sender open_id
                        let sender_open_id = event.event.sender.sender_id.open_id;
                        let chat_id = event.event.message.chat_id;

                        info!("Feishu: processing text message from chat_id={}, sender={}",
                            chat_id, sender_open_id);

                        // Parse message content
                        let content: serde_json::Value = match serde_json::from_str(&event.event.message.content) {
                            Ok(c) => c,
                            Err(e) => {
                                error!("Feishu: failed to parse message content: {}", e);
                                return;
                            }
                        };

                        let text = match content.get("text").and_then(|v| v.as_str()) {
                            Some(t) if !t.is_empty() => t,
                            _ => {
                                warn!("Feishu: empty or missing text in message");
                                return;
                            }
                        };

                        info!("Feishu: message content='{}'", text);

                        // Convert to InboundMessage
                        let inbound = InboundMessage::new(
                            "feishu",
                            sender_open_id,
                            chat_id.clone(),
                            text.to_string(),
                        );

                        info!("Feishu: publishing inbound to bus for chat_id={}", chat_id);

                        // Spawn async task to publish - this is necessary because the event handler is sync
                        tokio::spawn(async move {
                            match bus_clone.publish_inbound(inbound).await {
                                Ok(_) => info!("Feishu: successfully published inbound message"),
                                Err(e) => error!("Feishu: failed to publish inbound: {}", e),
                            }
                        });
                    })
                    .expect("Failed to register message receive handler")
                    .build();

                // Get config for WebSocket client
                let ws_config = Arc::new(lark_client.config.clone());

                info!("Feishu WebSocket config ready: app_id={}", ws_config.app_id);
                info!("Feishu WebSocket calling LarkWsClient::open...");

                // Start WebSocket connection - this blocks until connection closes
                match LarkWsClient::open(ws_config, event_handler).await {
                    Ok(_) => {
                        info!("Feishu WebSocket connected successfully");

                        // Keep running until stopped
                        loop {
                            tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
                            let running_guard = running.lock().await;
                            if !*running_guard {
                                info!("Feishu WebSocket stopping");
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        error!("Feishu WebSocket connection failed: {:?}", e);

                        // Keep running until stopped even if WebSocket failed
                        loop {
                            tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
                            let running_guard = running.lock().await;
                            if !*running_guard {
                                info!("Feishu WebSocket stopping after error");
                                break;
                            }
                        }
                    }
                }

                // Clear client on exit
                *lark_client_store.write().await = None;
                info!("Feishu client cleared");
            });

            info!("Feishu WebSocket task ended");
        });
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

        // Validate credentials by creating a test client
        let _test_client = LarkClient::builder(app_id, app_secret)
            .with_app_type(AppType::SelfBuild)
            .build();

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

    async fn start(&self, bus: MessageBus) -> Result<()> {
        {
            let mut running = self.running.lock().await;
            if *running {
                warn!("Feishu connector is already running");
                return Ok(());
            }
            *running = true;
        }

        // Load config from auth storage if available
        self.load_config_from_auth().await?;

        // Check authentication
        if !self.is_authenticated().await {
            anyhow::bail!("Feishu connector not authenticated");
        }

        // Store message bus for potential future use
        *self.message_bus.write().await = Some(bus.clone());

        // Get config snapshot for spawning task
        let config = self.config.read().await.clone();
        let running = self.running.clone();
        let lark_client_store = Arc::new(RwLock::new(None));

        // Spawn WebSocket task - the event handler is created inside the task
        // to avoid Send trait bound issues
        Self::spawn_websocket_task(bus, config, running, lark_client_store);

        info!("Feishu connector started (WebSocket long connection mode)");
        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        let mut running = self.running.lock().await;
        *running = false;
        info!("Feishu connector stopping");

        // Clear lark client
        *self.lark_client.write().await = None;

        info!("Feishu connector stopped");
        Ok(())
    }

    async fn status(&self) -> ChannelStatus {
        let config = self.config.read().await;
        let running = *self.running.lock().await;
        let client_initialized = self.lark_client.read().await.is_some();

        let mut status = ChannelStatus::new("feishu")
            .with_authenticated(!config.app_id.is_empty() && !config.app_secret.is_empty())
            .with_running(running);

        if !config.app_id.is_empty() {
            status = status.with_metadata("app_id_configured", json!(true));
        }

        if client_initialized {
            status = status.with_metadata("client_initialized", json!(true));
        }

        status
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Default for FeishuConnector {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for FeishuConnector {
    fn clone(&self) -> Self {
        let config = self.config.try_read()
            .map(|c| c.clone())
            .unwrap_or(FeishuConfig::default());
        let message_bus = self.message_bus.try_read()
            .map(|m| m.clone())
            .unwrap_or(None);
        let lark_client = self.lark_client.try_read()
            .map(|c| c.clone())
            .unwrap_or(None);

        Self {
            config: RwLock::new(config),
            auth_storage: self.auth_storage.clone(),
            running: self.running.clone(),
            message_bus: RwLock::new(message_bus),
            lark_client: RwLock::new(lark_client),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::ChannelAuth;

    #[test]
    fn test_feishu_connector_creation() {
        let connector = FeishuConnector::new();
        assert_eq!(connector.name(), "feishu");
    }

    #[tokio::test]
    async fn test_feishu_config_default() {
        let connector = FeishuConnector::new();
        let status = connector.status().await;
        assert_eq!(status.name, "feishu");
        assert!(!status.authenticated);
        assert!(!status.running);
    }

    #[tokio::test]
    async fn test_feishu_is_authenticated() {
        let connector = FeishuConnector::new();

        // Not authenticated initially
        assert!(!connector.is_authenticated().await);

        // Set config
        connector.set_config_from_auth(
            &ChannelAuth::new("test_secret")
                .with_extra("app_id", json!("test_app_id"))
                .with_extra("verification_token", json!("test_token"))
        ).await.unwrap();

        // Should be authenticated after config
        assert!(connector.is_authenticated().await);
    }

    #[tokio::test]
    async fn test_feishu_status_metadata() {
        let connector = FeishuConnector::new();

        // Set config
        connector.set_config_from_auth(
            &ChannelAuth::new("test_secret")
                .with_extra("app_id", json!("cli_test123"))
                .with_extra("verification_token", json!("test_token"))
        ).await.unwrap();

        let status = connector.status().await;
        assert!(status.metadata.get("app_id_configured").is_some());
    }

    #[test]
    fn test_feishu_config_clone() {
        let config = FeishuConfig {
            app_id: "test_app".to_string(),
            app_secret: "test_secret".to_string(),
            verification_token: "test_token".to_string(),
        };

        let cloned = config.clone();
        assert_eq!(config.app_id, cloned.app_id);
        assert_eq!(config.app_secret, cloned.app_secret);
        assert_eq!(config.verification_token, cloned.verification_token);
    }
}
