//! Channel manager for lifecycle management

use crate::auth::AuthStorage;
use crate::base::{ChannelEvent, ChannelStatus};
use crate::registry::ChannelRegistry;
use anyhow::Result;
use nanobot_bus::MessageBus;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tracing::{error, info, warn};

/// Manages channel lifecycle and MessageBus integration
pub struct ChannelManager {
    registry: Arc<ChannelRegistry>,
    auth_storage: Arc<AuthStorage>,
    running_channels: RwLock<HashMap<String, JoinHandle<()>>>,
    message_bus: Option<MessageBus>,
}

impl ChannelManager {
    /// Create a new channel manager (async)
    pub async fn new(registry: Arc<ChannelRegistry>, config_dir: impl AsRef<std::path::Path>) -> Result<Self> {
        let auth_storage = AuthStorage::new(config_dir).await?;

        Ok(Self {
            registry,
            auth_storage: Arc::new(auth_storage),
            running_channels: RwLock::new(HashMap::new()),
            message_bus: None,
        })
    }

    /// Set the message bus
    pub fn set_message_bus(&mut self, bus: MessageBus) {
        self.message_bus = Some(bus);
    }

    /// Get the message bus
    pub fn message_bus(&self) -> Option<&MessageBus> {
        self.message_bus.as_ref()
    }

    /// Get auth storage
    pub fn auth_storage(&self) -> &Arc<AuthStorage> {
        &self.auth_storage
    }

    /// Check if a channel is authenticated
    pub async fn is_authenticated(&self, channel_name: &str) -> bool {
        self.auth_storage.is_authenticated(channel_name).await
    }

    /// Authenticate a channel
    pub async fn authenticate(
        &self,
        _channel_name: &str,
        _config: &serde_json::Value,
    ) -> Result<()> {
        // Note: Authentication is handled by CLI storing credentials
        // This method is a placeholder for future enhancements
        Ok(())
    }

    /// Start a channel
    pub async fn start(&self, channel_name: &str) -> Result<()> {
        // Check if already running
        {
            let running = self.running_channels.read().await;
            if running.contains_key(channel_name) {
                warn!("Channel '{}' is already running", channel_name);
                return Ok(());
            }
        }

        // Check authentication
        if !self.is_authenticated(channel_name).await {
            anyhow::bail!(
                "Channel '{}' is not authenticated. Run 'rustbot channels login {}' first.",
                channel_name,
                channel_name
            );
        }

        // Get connector
        let connector = self
            .registry
            .get(channel_name)
            .await
            .ok_or_else(|| anyhow::anyhow!("Channel '{}' not found", channel_name))?;

        // Load auth data and set it to the connector
        if let Some(auth) = self.auth_storage.get_channel(channel_name).await {
            // Try to set config on Feishu connector
            #[cfg(feature = "feishu")]
            {
                if channel_name == "feishu" {
                    if let Some(feishu) = connector.as_any().downcast_ref::<crate::feishu::FeishuConnector>() {
                        let _ = feishu.set_config_from_auth(&auth).await;
                    }
                }
            }
        }

        // Get message bus
        let bus = self
            .message_bus
            .as_ref()
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("MessageBus not initialized"))?;

        // Start the channel in a background task
        let connector_clone = connector.clone();
        let channel_name_string = channel_name.to_string();
        let handle = tokio::spawn(async move {
            if let Err(e) = connector_clone.start(bus).await {
                error!("Channel '{}' error: {}", channel_name_string, e);
            }
        });

        // Track running channel
        {
            let mut running = self.running_channels.write().await;
            running.insert(channel_name.to_string(), handle);
        }

        info!("Channel '{}' started", channel_name);
        Ok(())
    }

    /// Stop a channel
    pub async fn stop(&self, channel_name: &str) -> Result<()> {
        // Get and remove the handle
        let handle = {
            let mut running = self.running_channels.write().await;
            running.remove(channel_name)
        };

        if let Some(handle) = handle {
            // Get connector and stop
            if let Some(connector) = self.registry.get(channel_name).await {
                if let Err(e) = connector.stop().await {
                    error!("Error stopping channel '{}': {}", channel_name, e);
                }
            }

            // Abort the task if still running
            handle.abort();
            info!("Channel '{}' stopped", channel_name);
        } else {
            warn!("Channel '{}' was not running", channel_name);
        }

        Ok(())
    }

    /// Stop all channels
    pub async fn stop_all(&self) {
        let channels: Vec<String> = {
            let running = self.running_channels.read().await;
            running.keys().cloned().collect()
        };

        for channel in channels {
            let _ = self.stop(&channel).await;
        }
    }

    /// Get status of all channels
    pub async fn status(&self) -> Vec<ChannelStatus> {
        let mut statuses = Vec::new();
        let channel_names = self.registry.list_names().await;

        for name in channel_names {
            if let Some(connector) = self.registry.get(&name).await {
                let mut status = connector.status().await;
                status.running = self.running_channels.read().await.contains_key(&name);
                status.authenticated = self.is_authenticated(&name).await;
                statuses.push(status);
            }
        }

        statuses
    }

    /// Get status of a single channel
    pub async fn get_channel_status(&self, channel_name: &str) -> Option<ChannelStatus> {
        let connector = self.registry.get(channel_name).await?;
        let mut status = connector.status().await;
        status.running = self.running_channels.read().await.contains_key(channel_name);
        status.authenticated = self.is_authenticated(channel_name).await;
        Some(status)
    }

    /// Publish a channel event (for logging/hooks)
    #[allow(dead_code)]
    fn publish_event(&self, event: ChannelEvent) {
        match &event {
            ChannelEvent::Started { name } => info!("Channel started: {}", name),
            ChannelEvent::Stopped { name } => info!("Channel stopped: {}", name),
            ChannelEvent::Authenticated { name } => info!("Channel authenticated: {}", name),
            ChannelEvent::AuthFailed { name, error } => {
                error!("Channel auth failed ({}): {}", name, error)
            }
            ChannelEvent::Error { name, error } => error!("Channel error ({}): {}", name, error),
            ChannelEvent::MessageReceived { name, chat_id } => {
                info!("Message received from {}/{}", name, chat_id)
            }
            ChannelEvent::MessageSent { name, chat_id } => {
                info!("Message sent to {}/{}", name, chat_id)
            }
        }
    }
}

impl Drop for ChannelManager {
    fn drop(&mut self) {
        // Attempt to stop all channels (best effort)
        // Note: Can't use async in Drop, so channels will be cleaned up
        // when the tokio runtime shuts down
        info!("ChannelManager dropped - channels will be cleaned up by runtime");
    }
}
