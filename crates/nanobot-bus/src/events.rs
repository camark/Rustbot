//! Message events for the message bus

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// Inbound message from a channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboundMessage {
    /// Unique message ID
    #[serde(default = "generate_message_id")]
    pub id: String,

    /// Timestamp
    #[serde(default = "Utc::now")]
    pub timestamp: DateTime<Utc>,

    /// Source channel name
    pub channel: String,

    /// Sender identifier
    pub sender_id: String,

    /// Chat/channel identifier
    pub chat_id: String,

    /// Message content
    pub content: String,

    /// Optional media attachments
    #[serde(default)]
    pub media: Vec<String>,

    /// Optional metadata
    #[serde(default)]
    pub metadata: serde_json::Map<String, Value>,

    /// Optional session key override
    #[serde(skip)]
    pub session_key_override: Option<String>,
}

fn generate_message_id() -> String {
    Uuid::new_v4().to_string()
}

impl InboundMessage {
    /// Create a new inbound message
    pub fn new(
        channel: impl Into<String>,
        sender_id: impl Into<String>,
        chat_id: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        Self {
            id: generate_message_id(),
            timestamp: Utc::now(),
            channel: channel.into(),
            sender_id: sender_id.into(),
            chat_id: chat_id.into(),
            content: content.into(),
            media: Vec::new(),
            metadata: serde_json::Map::new(),
            session_key_override: None,
        }
    }

    /// Get the session key
    pub fn session_key(&self) -> String {
        if let Some(override_key) = &self.session_key_override {
            return override_key.clone();
        }
        format!("{}:{}", self.channel, self.chat_id)
    }

    /// Set metadata field
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<Value>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Check if streaming is requested
    pub fn wants_streaming(&self) -> bool {
        self.metadata
            .get("_wants_stream")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }
}

/// Outbound message to a channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboundMessage {
    /// Unique message ID
    #[serde(default = "generate_message_id")]
    pub id: String,

    /// Timestamp
    #[serde(default = "Utc::now")]
    pub timestamp: DateTime<Utc>,

    /// Target channel name
    pub channel: String,

    /// Target chat identifier
    pub chat_id: String,

    /// Message content
    pub content: String,

    /// Optional metadata
    #[serde(default)]
    pub metadata: serde_json::Map<String, Value>,
}

impl OutboundMessage {
    /// Create a new outbound message
    pub fn new(
        channel: impl Into<String>,
        chat_id: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        Self {
            id: generate_message_id(),
            timestamp: Utc::now(),
            channel: channel.into(),
            chat_id: chat_id.into(),
            content: content.into(),
            metadata: serde_json::Map::new(),
        }
    }

    /// Set metadata field
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<Value>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Check if this is a stream delta
    pub fn is_stream_delta(&self) -> bool {
        self.metadata
            .get("_stream_delta")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }

    /// Check if this is a stream end marker
    pub fn is_stream_end(&self) -> bool {
        self.metadata
            .get("_stream_end")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }

    /// Check if this is a progress update
    pub fn is_progress(&self) -> bool {
        self.metadata
            .get("_progress")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }

    /// Check if this is a tool hint
    pub fn is_tool_hint(&self) -> bool {
        self.metadata
            .get("_tool_hint")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }
}
