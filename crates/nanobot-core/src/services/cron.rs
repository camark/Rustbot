//! Cron service for scheduling periodic tasks

use anyhow::Result;
use cron::Schedule;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tracing::{info, error, warn};

/// Cron service for scheduling periodic tasks
pub struct CronService {
    jobs: Arc<RwLock<HashMap<String, CronJob>>>,
    running: Arc<RwLock<bool>>,
}

struct CronJob {
    name: String,
    schedule: Schedule,
    action: JobAction,
    handle: Option<JoinHandle<()>>,
}

/// Job action type - boxed async function
type JobAction = Box<dyn Fn() -> std::pin::Pin<Box<dyn futures::Future<Output = ()> + Send>> + Send + Sync>;

// Manual Send/Sync impl since Box<dyn Fn...> doesn't impl them
unsafe impl Send for CronJob {}
unsafe impl Sync for CronJob {}

impl CronService {
    /// Create a new cron service
    pub fn new() -> Self {
        Self {
            jobs: Arc::new(RwLock::new(HashMap::new())),
            running: Arc::new(RwLock::new(false)),
        }
    }

    /// Register a new cron job
    pub async fn add_job<F, Fut>(
        &self,
        name: impl Into<String>,
        cron_expression: &str,
        action: F,
    ) -> Result<()>
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        let schedule: Schedule = cron_expression
            .parse()
            .map_err(|e| anyhow::anyhow!("Invalid cron expression '{}': {}", cron_expression, e))?;

        let name = name.into();
        info!("Adding cron job '{}' with schedule '{}'", name, cron_expression);

        let job = CronJob {
            name: name.clone(),
            schedule,
            action: Box::new(move || Box::pin(action())),
            handle: None,
        };

        self.jobs.write().await.insert(name, job);
        Ok(())
    }

    /// Remove a cron job
    pub async fn remove_job(&self, name: &str) -> Result<()> {
        let mut jobs = self.jobs.write().await;
        if let Some(mut job) = jobs.remove(name) {
            if let Some(handle) = job.handle.take() {
                handle.abort();
            }
            info!("Removed cron job '{}'", name);
        } else {
            warn!("Cron job '{}' not found", name);
        }
        Ok(())
    }

    /// List all cron jobs
    pub async fn list_jobs(&self) -> Vec<String> {
        let jobs = self.jobs.read().await;
        jobs.keys().cloned().collect()
    }

    /// Get job info
    pub async fn get_job(&self, name: &str) -> Option<String> {
        let jobs = self.jobs.read().await;
        jobs.get(name).map(|j| j.schedule.to_string())
    }

    /// Start the cron service
    pub async fn start(&self) -> Result<()> {
        {
            let mut running = self.running.write().await;
            if *running {
                warn!("Cron service is already running");
                return Ok(());
            }
            *running = true;
        }

        info!("Starting cron service");

        // Start all registered jobs
        let jobs = self.jobs.clone();
        let running = self.running.clone();

        let handle = tokio::spawn(async move {
            while *running.read().await {
                let now = chrono::Utc::now();
                let mut jobs_to_run = Vec::new();

                // Check which jobs should run
                for (name, job) in jobs.read().await.iter() {
                    if job.schedule.upcoming(chrono::Utc).next().unwrap() <= now {
                        jobs_to_run.push(name.clone());
                    }
                }

                // Execute jobs
                for name in jobs_to_run {
                    info!("Executing cron job '{}'", name);

                    let mut jobs = jobs.write().await;
                    if let Some(job) = jobs.get_mut(&name) {
                        // Execute the job action
                        let future = (job.action)();
                        tokio::spawn(async move {
                            future.await;
                            info!("Cron job '{}' completed", name);
                        });
                    }
                }

                // Sleep for a second before checking again
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            }
        });

        // Store handle in jobs
        // Note: This is simplified - in production would store handles properly
        let _ = handle;

        Ok(())
    }

    /// Stop the cron service
    pub async fn stop(&self) {
        let mut running = self.running.write().await;
        *running = false;
        info!("Cron service stopped");
    }

    /// Check if running
    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }
}

impl Default for CronService {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper to create a cron schedule from expression
pub fn parse_schedule(expression: &str) -> Result<Schedule> {
    expression
        .parse()
        .map_err(|e| anyhow::anyhow!("Invalid cron expression: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_schedule() {
        assert!(parse_schedule("*/5 * * * * *").is_ok());
        assert!(parse_schedule("invalid").is_err());
    }

    #[tokio::test]
    async fn test_cron_service() {
        let service = CronService::new();
        service.add_job("test", "*/5 * * * * *", || async {
            println!("Job executed");
        }).await.unwrap();

        assert!(service.list_jobs().await.contains(&"test".to_string()));

        service.remove_job("test").await.unwrap();
        assert!(!service.list_jobs().await.contains(&"test".to_string()));
    }
}
