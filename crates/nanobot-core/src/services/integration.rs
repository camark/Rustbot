//! Service integration module
//!
//! This module provides integration between various services:
//! - Cron service with AgentLoop
//! - Heartbeat service with SessionManager
//! - API server with MessageBus and AgentLoop

use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, error};

use crate::{AgentLoop, session::SessionManager};
use nanobot_bus::MessageBus;

use super::{CronService, HeartbeatService};

/// Service manager for coordinating all background services
pub struct ServiceManager {
    cron: Arc<CronService>,
    heartbeat: Arc<HeartbeatService>,
    agent_loop: Arc<AgentLoop>,
    running: Arc<RwLock<bool>>,
}

impl ServiceManager {
    /// Create a new service manager
    pub fn new(
        cron: Arc<CronService>,
        heartbeat: Arc<HeartbeatService>,
        agent_loop: Arc<AgentLoop>,
    ) -> Self {
        Self {
            cron,
            heartbeat,
            agent_loop,
            running: Arc::new(RwLock::new(false)),
        }
    }

    /// Start all services
    pub async fn start_all(&self) -> Result<(), anyhow::Error> {
        {
            let mut running = self.running.write().await;
            if *running {
                return Err(anyhow::anyhow!("Services are already running"));
            }
            *running = true;
        }

        info!("Starting all services...");

        // Start heartbeat service
        self.heartbeat.start().await?;
        info!("Heartbeat service started");

        // Start cron service
        self.cron.start().await?;
        info!("Cron service started");

        // Start agent loop
        let agent_loop = self.agent_loop.clone();
        tokio::spawn(async move {
            if let Err(e) = agent_loop.run().await {
                error!("Agent loop error: {}", e);
            }
        });
        info!("Agent loop started");

        info!("All services started successfully");
        Ok(())
    }

    /// Stop all services
    pub async fn stop_all(&self) {
        let mut running = self.running.write().await;
        if !*running {
            return;
        }
        *running = false;

        info!("Stopping all services...");

        // Stop agent loop
        self.agent_loop.stop().await;
        info!("Agent loop stopped");

        // Stop heartbeat service
        self.heartbeat.stop().await;
        info!("Heartbeat service stopped");

        // Stop cron service
        self.cron.stop().await;
        info!("Cron service stopped");

        info!("All services stopped");
    }

    /// Check if any service is running
    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }

    /// Get service status
    pub async fn status(&self) -> ServiceStatus {
        ServiceStatus {
            running: self.is_running().await,
            cron_running: self.cron.is_running().await,
            heartbeat_running: self.heartbeat.is_running().await,
            agent_running: self.agent_loop.is_running().await,
        }
    }
}

/// Service status information
#[derive(Debug, Clone)]
pub struct ServiceStatus {
    pub running: bool,
    pub cron_running: bool,
    pub heartbeat_running: bool,
    pub agent_running: bool,
}

/// Helper to create and configure services
pub struct ServiceBuilder {
    #[allow(dead_code)]
    message_bus: MessageBus,
    session_manager: Arc<SessionManager>,
}

impl ServiceBuilder {
    /// Create a new service builder
    pub fn new(message_bus: MessageBus, session_manager: Arc<SessionManager>) -> Self {
        Self {
            message_bus,
            session_manager,
        }
    }

    /// Build cron service with default configuration
    pub fn build_cron(&self) -> Arc<CronService> {
        Arc::new(CronService::new())
    }

    /// Build heartbeat service with default configuration
    pub fn build_heartbeat(&self) -> Arc<HeartbeatService> {
        Arc::new(HeartbeatService::with_defaults(self.session_manager.clone()))
    }

    /// Build service manager with all services
    pub fn build_manager(
        &self,
        agent_loop: Arc<AgentLoop>,
    ) -> ServiceManager {
        let cron = self.build_cron();
        let heartbeat = self.build_heartbeat();

        ServiceManager::new(cron, heartbeat, agent_loop)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_environment() -> (MessageBus, Arc<SessionManager>, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let bus = MessageBus::new();
        let session_manager = Arc::new(SessionManager::new(&temp_dir).unwrap());
        (bus, session_manager, temp_dir)
    }

    #[tokio::test]
    async fn test_service_builder() {
        let (bus, session_manager, _temp_dir) = create_test_environment();
        let builder = ServiceBuilder::new(bus, session_manager);

        let cron = builder.build_cron();
        let heartbeat = builder.build_heartbeat();

        assert!(!cron.is_running().await);
        assert!(!heartbeat.is_running().await);
    }
}
