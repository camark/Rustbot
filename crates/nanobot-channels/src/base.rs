//! Base types and traits for channel connectors

use anyhow::Result;
use async_trait::async_trait;
use nanobot_bus::MessageBus;
use serde::{Deserialize, Serialize};

/// Channel connector trait - all channels must implement this
#[async_trait]
pub trait ChannelConnector: Send + Sync {
    /// Channel name (e.g., "telegram", "discord")
    fn name(&self) -> &str;

    /// Check if channel is authenticated
    async fn is_authenticated(&self) -> bool;

    /// Authenticate the channel (interactive or token-based)
    async fn authenticate(&mut self, config: &serde_json::Value) -> Result<()>;

    /// Start receiving messages and publishing to MessageBus
    async fn start(&self, bus: MessageBus) -> Result<()>;

    /// Stop the channel
    async fn stop(&self) -> Result<()>;

    /// Get channel status
    async fn status(&self) -> ChannelStatus;
}

/// Channel status information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelStatus {
    /// Channel name
    pub name: String,

    /// Whether the channel is authenticated
    pub authenticated: bool,

    /// Whether the channel is currently running
    pub running: bool,

    /// Status message or error description
    pub message: Option<String>,

    /// Additional channel-specific metadata
    #[serde(default)]
    pub metadata: serde_json::Map<String, serde_json::Value>,
}

impl ChannelStatus {
    /// Create a new channel status
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            authenticated: false,
            running: false,
            message: None,
            metadata: serde_json::Map::new(),
        }
    }

    /// Set authentication status
    pub fn with_authenticated(mut self, authenticated: bool) -> Self {
        self.authenticated = authenticated;
        self
    }

    /// Set running status
    pub fn with_running(mut self, running: bool) -> Self {
        self.running = running;
        self
    }

    /// Set status message
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }

    /// Add metadata
    pub fn with_metadata(
        mut self,
        key: impl Into<String>,
        value: impl Into<serde_json::Value>,
    ) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

/// Channel event types
#[derive(Debug, Clone)]
pub enum ChannelEvent {
    /// Channel started successfully
    Started { name: String },

    /// Channel stopped
    Stopped { name: String },

    /// Channel authentication succeeded
    Authenticated { name: String },

    /// Channel authentication failed
    AuthFailed { name: String, error: String },

    /// Channel error occurred
    Error { name: String, error: String },

    /// Message received from channel
    MessageReceived { name: String, chat_id: String },

    /// Message sent to channel
    MessageSent { name: String, chat_id: String },
}
