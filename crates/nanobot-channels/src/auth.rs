//! Authentication storage for channel credentials

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tokio::sync::RwLock;
use tracing::{debug, info};

/// Authentication storage for channel credentials
pub struct AuthStorage {
    file_path: PathBuf,
    data: RwLock<AuthData>,
}

/// Authentication data structure
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AuthData {
    /// Channel credentials (token, app_secret, etc.)
    #[serde(default)]
    pub channels: HashMap<String, ChannelAuth>,
}

/// Single channel authentication data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelAuth {
    /// Access token or bot token
    pub token: String,

    /// Optional refresh token (for OAuth flows)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,

    /// Token expiry timestamp (Unix timestamp)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<u64>,

    /// Additional channel-specific data
    #[serde(default, flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

impl AuthStorage {
    /// Create or load authentication storage
    pub async fn new(config_dir: impl AsRef<Path>) -> Result<Self> {
        let file_path = config_dir.as_ref().join("auth.json");

        // Create config directory if it doesn't exist
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).context("Failed to create config directory")?;
        }

        // Load existing data or create new
        let data = if file_path.exists() {
            let content = fs::read_to_string(&file_path).context("Failed to read auth file")?;
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            AuthData::default()
        };

        Ok(Self { file_path, data: RwLock::new(data) })
    }

    /// Get channel authentication data
    pub async fn get_channel(&self, channel_name: &str) -> Option<ChannelAuth> {
        let data = self.data.read().await;
        data.channels.get(channel_name).cloned()
    }

    /// Get channel token
    pub async fn get_token(&self, channel_name: &str) -> Option<String> {
        let data = self.data.read().await;
        data.channels
            .get(channel_name)
            .map(|auth| auth.token.clone())
    }

    /// Set channel authentication
    pub async fn set_channel(&self, channel_name: impl Into<String>, auth: ChannelAuth) -> Result<()> {
        let channel_name = channel_name.into();
        info!("Storing authentication for channel: {}", channel_name);
        let mut data = self.data.write().await;
        data.channels.insert(channel_name, auth);
        drop(data);
        self.save().await
    }

    /// Remove channel authentication
    pub async fn remove_channel(&self, channel_name: &str) -> Result<()> {
        info!("Removing authentication for channel: {}", channel_name);
        let mut data = self.data.write().await;
        data.channels.remove(channel_name);
        drop(data);
        self.save().await
    }

    /// Check if channel is authenticated
    pub async fn is_authenticated(&self, channel_name: &str) -> bool {
        let data = self.data.read().await;
        if let Some(auth) = data.channels.get(channel_name) {
            // Check if token exists and is not expired
            if auth.token.is_empty() {
                return false;
            }
            if let Some(expires_at) = auth.expires_at {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                return now < expires_at;
            }
            true
        } else {
            false
        }
    }

    /// Save authentication data to disk
    async fn save(&self) -> Result<()> {
        let data = self.data.read().await;
        let content = serde_json::to_string_pretty(&*data)
            .context("Failed to serialize auth data")?;
        drop(data);
        fs::write(&self.file_path, content).context("Failed to write auth file")?;
        debug!("Authentication data saved to {:?}", self.file_path);

        // Set secure file permissions on Unix-like systems
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(metadata) = fs::metadata(&self.file_path) {
                let mut permissions = metadata.permissions();
                permissions.set_mode(0o600); // Owner read/write only
                let _ = fs::set_permissions(&self.file_path, permissions);
            }
        }

        Ok(())
    }

    /// Get the auth file path
    pub fn file_path(&self) -> &Path {
        &self.file_path
    }
}

/// Helper to create channel auth
impl ChannelAuth {
    /// Create a new channel auth with token
    pub fn new(token: impl Into<String>) -> Self {
        Self {
            token: token.into(),
            refresh_token: None,
            expires_at: None,
            extra: HashMap::new(),
        }
    }

    /// Set refresh token
    pub fn with_refresh_token(mut self, token: impl Into<String>) -> Self {
        self.refresh_token = Some(token.into());
        self
    }

    /// Set expiry
    pub fn with_expiry(mut self, expires_at: u64) -> Self {
        self.expires_at = Some(expires_at);
        self
    }

    /// Set extra data
    pub fn with_extra(
        mut self,
        key: impl Into<String>,
        value: impl Into<serde_json::Value>,
    ) -> Self {
        self.extra.insert(key.into(), value.into());
        self
    }
}
