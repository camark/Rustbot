//! API authentication

use std::sync::Arc;
use tokio::sync::RwLock;

/// API Key authentication manager
pub struct ApiAuth {
    keys: RwLock<Vec<String>>,
}

impl ApiAuth {
    /// Create a new auth manager with no keys
    pub fn new() -> Self {
        Self {
            keys: RwLock::new(Vec::new()),
        }
    }

    /// Create with a single API key
    pub fn with_key(key: impl Into<String>) -> Self {
        Self {
            keys: RwLock::new(vec![key.into()]),
        }
    }

    /// Create with multiple API keys
    pub fn with_keys(keys: Vec<String>) -> Self {
        Self {
            keys: RwLock::new(keys),
        }
    }

    /// Check if an API key is valid
    pub async fn is_valid(&self, key: &str) -> bool {
        let keys = self.keys.read().await;
        keys.iter().any(|k| k == key)
    }

    /// Add a new API key
    pub async fn add_key(&self, key: impl Into<String>) {
        let mut keys = self.keys.write().await;
        keys.push(key.into());
    }

    /// Remove an API key
    pub async fn remove_key(&self, key: &str) {
        let mut keys = self.keys.write().await;
        keys.retain(|k| k != key);
    }

    /// Check if authentication is required (no keys = no auth)
    pub async fn is_enabled(&self) -> bool {
        !self.keys.read().await.is_empty()
    }
}

impl Default for ApiAuth {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract API key from Authorization header
pub fn extract_api_key(auth_header: Option<&str>) -> Option<String> {
    auth_header
        .and_then(|h| h.strip_prefix("Bearer "))
        .map(String::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_api_auth() {
        let auth = ApiAuth::with_key("test-key");
        assert!(auth.is_valid("test-key").await);
        assert!(!auth.is_valid("wrong-key").await);
    }

    #[tokio::test]
    async fn test_add_remove_key() {
        let auth = ApiAuth::new();
        auth.add_key("key1").await;
        assert!(auth.is_valid("key1").await);
        auth.remove_key("key1").await;
        assert!(!auth.is_valid("key1").await);
    }
}
