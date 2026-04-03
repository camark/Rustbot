//! Heartbeat service for session cleanup and health monitoring

use anyhow::Result;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{RwLock, watch};
use tracing::{info, warn, error};

use crate::session::SessionManager;

/// Heartbeat service configuration
#[derive(Debug, Clone)]
pub struct HeartbeatConfig {
    /// Interval between heartbeats
    pub interval: Duration,
    /// Max session age in days before cleanup
    pub max_session_age_days: u32,
    /// Enable session consolidation
    pub enable_consolidation: bool,
    /// Consolidation threshold (messages)
    pub consolidation_threshold: usize,
}

impl Default for HeartbeatConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(30 * 60), // 30 minutes
            max_session_age_days: 30,
            enable_consolidation: true,
            consolidation_threshold: 50,
        }
    }
}

/// Heartbeat service for periodic maintenance tasks
pub struct HeartbeatService {
    config: HeartbeatConfig,
    session_manager: Arc<SessionManager>,
    running: Arc<RwLock<bool>>,
    shutdown_tx: watch::Sender<bool>,
}

impl HeartbeatService {
    /// Create a new heartbeat service
    pub fn new(config: HeartbeatConfig, session_manager: Arc<SessionManager>) -> Self {
        let (shutdown_tx, _) = watch::channel(false);

        Self {
            config,
            session_manager,
            running: Arc::new(RwLock::new(false)),
            shutdown_tx,
        }
    }

    /// Create with default config
    pub fn with_defaults(session_manager: Arc<SessionManager>) -> Self {
        Self::new(HeartbeatConfig::default(), session_manager)
    }

    /// Start the heartbeat service
    pub async fn start(&self) -> Result<()> {
        {
            let mut running = self.running.write().await;
            if *running {
                warn!("Heartbeat service is already running");
                return Ok(());
            }
            *running = true;
        }

        info!(
            "Starting heartbeat service (interval: {:?}, max_session_age: {} days)",
            self.config.interval, self.config.max_session_age_days
        );

        let session_manager = self.session_manager.clone();
        let config = self.config.clone();
        let running = self.running.clone();
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = tokio::time::sleep(config.interval) => {
                        // Run maintenance tasks
                        info!("Running heartbeat maintenance tasks");

                        // Cleanup expired sessions
                        match session_manager.cleanup_expired(config.max_session_age_days) {
                            Ok(count) => {
                                if count > 0 {
                                    info!("Cleaned up {} expired sessions", count);
                                }
                            }
                            Err(e) => {
                                error!("Failed to cleanup sessions: {}", e);
                            }
                        }

                        // Consolidate old sessions if enabled
                        if config.enable_consolidation {
                            let consolidated = session_manager.consolidate_old_sessions(
                                config.consolidation_threshold,
                                &|session| {
                                    // In production, this would call an LLM to generate a summary
                                    // For now, just return a placeholder
                                    Some(format!(
                                        "Session summary: {} messages from {}",
                                        session.messages.len(),
                                        session.created_at
                                    ))
                                },
                            );
                            if consolidated > 0 {
                                info!("Consolidated {} sessions", consolidated);
                            }
                        }
                    }
                    _ = shutdown_rx.changed() => {
                        if *shutdown_rx.borrow() {
                            info!("Heartbeat service received shutdown signal");
                            break;
                        }
                    }
                }
            }

            info!("Heartbeat service loop exited");
        });

        Ok(())
    }

    /// Stop the heartbeat service
    pub async fn stop(&self) {
        let mut running = self.running.write().await;
        *running = false;

        // Send shutdown signal
        let _ = self.shutdown_tx.send(true);

        info!("Heartbeat service stopped");
    }

    /// Check if running
    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }

    /// Trigger an immediate maintenance run
    pub async fn run_maintenance(&self) -> Result<()> {
        info!("Running immediate maintenance");

        // Cleanup expired sessions
        let count = self.session_manager.cleanup_expired(self.config.max_session_age_days)?;
        info!("Cleaned up {} expired sessions", count);

        // Consolidate old sessions
        if self.config.enable_consolidation {
            let consolidated = self.session_manager.consolidate_old_sessions(
                self.config.consolidation_threshold,
                &|session| {
                    Some(format!(
                        "Session summary: {} messages from {}",
                        session.messages.len(),
                        session.created_at
                    ))
                },
            );
            info!("Consolidated {} sessions", consolidated);
        }

        Ok(())
    }

    /// Get service status
    pub async fn status(&self) -> HeartbeatStatus {
        HeartbeatStatus {
            running: self.is_running().await,
            interval_secs: self.config.interval.as_secs(),
            max_session_age_days: self.config.max_session_age_days,
            consolidation_enabled: self.config.enable_consolidation,
        }
    }
}

/// Heartbeat service status
#[derive(Debug, Clone)]
pub struct HeartbeatStatus {
    pub running: bool,
    pub interval_secs: u64,
    pub max_session_age_days: u32,
    pub consolidation_enabled: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn create_test_session_manager() -> (Arc<SessionManager>, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::new(&temp_dir).unwrap();
        (Arc::new(manager), temp_dir)
    }

    #[tokio::test]
    async fn test_heartbeat_service() {
        let (session_manager, _temp_dir) = create_test_session_manager();
        let service = HeartbeatService::with_defaults(session_manager);

        assert!(!service.is_running().await);

        service.start().await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        assert!(service.is_running().await);

        service.stop().await;
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        assert!(!service.is_running().await);
    }

    #[tokio::test]
    async fn test_heartbeat_status() {
        let (session_manager, _temp_dir) = create_test_session_manager();
        let service = HeartbeatService::with_defaults(session_manager);

        let status = service.status().await;
        assert!(!status.running);
        assert_eq!(status.interval_secs, 30 * 60);
        assert_eq!(status.max_session_age_days, 30);
        assert!(status.consolidation_enabled);
    }
}
