//! Channel registry for managing available connectors

use crate::base::ChannelConnector;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Registry of available channel connectors
pub struct ChannelRegistry {
    connectors: RwLock<HashMap<String, Arc<dyn ChannelConnector>>>,
}

impl ChannelRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            connectors: RwLock::new(HashMap::new()),
        }
    }

    /// Register a channel connector
    pub async fn register(&self, connector: Arc<dyn ChannelConnector>) {
        let name = connector.name().to_string();
        let mut connectors = self.connectors.write().await;
        connectors.insert(name, connector);
    }

    /// Get a connector by name
    pub async fn get(&self, name: &str) -> Option<Arc<dyn ChannelConnector>> {
        let connectors = self.connectors.read().await;
        connectors.get(name).cloned()
    }

    /// List all registered connector names
    pub async fn list_names(&self) -> Vec<String> {
        let connectors = self.connectors.read().await;
        connectors.keys().cloned().collect()
    }

    /// Check if a connector is registered
    pub async fn contains(&self, name: &str) -> bool {
        let connectors = self.connectors.read().await;
        connectors.contains_key(name)
    }

    /// Remove a connector
    pub async fn remove(&self, name: &str) -> Option<Arc<dyn ChannelConnector>> {
        let mut connectors = self.connectors.write().await;
        connectors.remove(name)
    }

    /// Get all connectors
    pub async fn get_all(&self) -> Vec<Arc<dyn ChannelConnector>> {
        let connectors = self.connectors.read().await;
        connectors.values().cloned().collect()
    }

    /// Get connector count
    pub async fn len(&self) -> usize {
        let connectors = self.connectors.read().await;
        connectors.len()
    }

    /// Check if registry is empty
    pub async fn is_empty(&self) -> bool {
        let connectors = self.connectors.read().await;
        connectors.is_empty()
    }
}

impl Default for ChannelRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper to create a registry with default connectors
pub fn create_default_registry() -> Arc<ChannelRegistry> {
    let registry = Arc::new(ChannelRegistry::new());

    // Register available connectors based on features
    #[cfg(feature = "telegram")]
    {
        let connector = Arc::new(crate::telegram::TelegramConnector::new());
        let registry_clone = registry.clone();
        tokio::spawn(async move {
            registry_clone.register(connector).await;
        });
    }

    #[cfg(feature = "discord")]
    {
        let connector = Arc::new(crate::discord::DiscordConnector::new());
        let registry_clone = registry.clone();
        tokio::spawn(async move {
            registry_clone.register(connector).await;
        });
    }

    #[cfg(feature = "feishu")]
    {
        let connector = Arc::new(crate::feishu::FeishuConnector::new());
        let registry_clone = registry.clone();
        tokio::spawn(async move {
            registry_clone.register(connector).await;
        });
    }

    registry
}
