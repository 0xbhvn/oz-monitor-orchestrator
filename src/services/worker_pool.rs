//! Monitor Worker Pool
//!
//! Manages a pool of OpenZeppelin Monitor instances, each handling
//! a subset of tenant configurations.

use anyhow::Result;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, instrument, warn};
use uuid::Uuid;

// Import OpenZeppelin Monitor types
use openzeppelin_monitor::{
    models::Monitor,
    repositories::MonitorRepositoryTrait,
    services::{
        blockchain::ClientPoolTrait,
    },
};

use crate::repositories::TenantAwareMonitorRepository;
use crate::services::block_cache::BlockCacheService;

/// Worker configuration
#[derive(Debug, Clone)]
pub struct WorkerConfig {
    /// Maximum number of tenants per worker
    pub max_tenants_per_worker: usize,
    /// Worker health check interval
    pub health_check_interval: std::time::Duration,
    /// Tenant reload interval
    pub tenant_reload_interval: std::time::Duration,
}

impl Default for WorkerConfig {
    fn default() -> Self {
        Self {
            max_tenants_per_worker: 50,
            health_check_interval: std::time::Duration::from_secs(30),
            tenant_reload_interval: std::time::Duration::from_secs(300), // 5 minutes
        }
    }
}

/// Individual monitor worker
pub struct MonitorWorker {
    pub id: String,
    pub assigned_tenants: Arc<RwLock<Vec<Uuid>>>,
    pub status: Arc<RwLock<WorkerStatus>>,
    db: Arc<PgPool>,
    cache: Arc<BlockCacheService>,
    config: WorkerConfig,
}

#[derive(Debug, Clone)]
pub enum WorkerStatus {
    Starting,
    Running,
    Reloading,
    Stopping,
    Stopped,
    Error(String),
}

impl MonitorWorker {
    pub fn new(
        id: String,
        db: Arc<PgPool>,
        cache: Arc<BlockCacheService>,
        config: WorkerConfig,
    ) -> Self {
        Self {
            id,
            assigned_tenants: Arc::new(RwLock::new(Vec::new())),
            status: Arc::new(RwLock::new(WorkerStatus::Starting)),
            db,
            cache,
            config,
        }
    }

    /// Assign tenants to this worker
    pub async fn assign_tenants(&self, tenant_ids: Vec<Uuid>) {
        let mut tenants = self.assigned_tenants.write().await;
        *tenants = tenant_ids;
        info!("Worker {} assigned {} tenants", self.id, tenants.len());
    }

    /// Start the worker
    #[instrument(skip(self, client_pool), fields(worker_id = %self.id))]
    pub async fn start<CP: ClientPoolTrait + Send + Sync + 'static>(
        &self,
        client_pool: Arc<CP>,
    ) -> Result<()> {
        *self.status.write().await = WorkerStatus::Running;
        info!("Starting worker {}", self.id);

        // Start background tasks
        let health_handle = self.start_health_check();
        let reload_handle = self.start_tenant_reload();
        let monitor_handle = self.start_monitoring(client_pool).await?;

        // Wait for any task to complete (they should run forever)
        tokio::select! {
            _ = health_handle => warn!("Health check task stopped"),
            _ = reload_handle => warn!("Tenant reload task stopped"),
            _ = monitor_handle => warn!("Monitor task stopped"),
        }

        *self.status.write().await = WorkerStatus::Stopped;
        Ok(())
    }

    /// Start health check task
    fn start_health_check(&self) -> tokio::task::JoinHandle<()> {
        let status = self.status.clone();
        let interval = self.config.health_check_interval;
        let worker_id = self.id.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(interval);
            loop {
                interval.tick().await;
                let current_status = status.read().await.clone();
                info!("Worker {} health check: {:?}", worker_id, current_status);
            }
        })
    }

    /// Start tenant reload task
    fn start_tenant_reload(&self) -> tokio::task::JoinHandle<()> {
        let status = self.status.clone();
        let interval = self.config.tenant_reload_interval;
        let worker_id = self.id.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(interval);
            loop {
                interval.tick().await;
                info!("Worker {} reloading tenant configurations", worker_id);
                *status.write().await = WorkerStatus::Reloading;
                // Actual reload logic would go here
                *status.write().await = WorkerStatus::Running;
            }
        })
    }

    /// Start monitoring task
    async fn start_monitoring<CP: ClientPoolTrait + Send + Sync + 'static>(
        &self,
        client_pool: Arc<CP>,
    ) -> Result<tokio::task::JoinHandle<()>> {
        let tenants = self.assigned_tenants.clone();
        let db = self.db.clone();
        let cache = self.cache.clone();
        let worker_id = self.id.clone();
        let status = self.status.clone();

        let handle = tokio::spawn(async move {
            loop {
                let tenant_ids = tenants.read().await.clone();
                if tenant_ids.is_empty() {
                    tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
                    continue;
                }

                info!(
                    "Worker {} processing {} tenants",
                    worker_id,
                    tenant_ids.len()
                );

                // Create repositories for these tenants
                let monitor_repo =
                    TenantAwareMonitorRepository::new(db.clone(), tenant_ids.clone());
                let monitors = monitor_repo.get_all();

                if monitors.is_empty() {
                    tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
                    continue;
                }

                // Process each monitor
                for (name, monitor) in monitors {
                    match process_monitor(&monitor, &cache, &client_pool).await {
                        Ok(_) => info!("Worker {} processed monitor {}", worker_id, name),
                        Err(e) => {
                            error!(
                                "Worker {} failed to process monitor {}: {}",
                                worker_id, name, e
                            );
                            *status.write().await = WorkerStatus::Error(e.to_string());
                        }
                    }
                }

                // Sleep before next iteration
                tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
            }
        });

        Ok(handle)
    }
}

/// Process a single monitor
async fn process_monitor<CP: ClientPoolTrait>(
    monitor: &Monitor,
    _cache: &Arc<BlockCacheService>,
    _client_pool: &Arc<CP>,
) -> Result<()> {
    // This is a simplified version - in reality you'd integrate with OZ monitor's
    // full processing pipeline
    info!("Processing monitor: {}", monitor.name);

    // Process each network the monitor is configured for
    for network_slug in &monitor.networks {
        info!(
            "Processing network {} for monitor {}",
            network_slug, monitor.name
        );

        // In a real implementation, you'd:
        // 1. Get the network configuration
        // 2. Create a cached client for the network
        // 3. Use OZ monitor's filter service to process blocks
        // 4. Send notifications for matches
    }

    Ok(())
}

/// Monitor worker pool manager
pub struct MonitorWorkerPool {
    workers: Arc<RwLock<HashMap<String, Arc<MonitorWorker>>>>,
    db: Arc<PgPool>,
    cache: Arc<BlockCacheService>,
    config: WorkerConfig,
}

impl MonitorWorkerPool {
    pub fn new(db: Arc<PgPool>, cache: Arc<BlockCacheService>, config: WorkerConfig) -> Self {
        Self {
            workers: Arc::new(RwLock::new(HashMap::new())),
            db,
            cache,
            config,
        }
    }

    /// Create and start a new worker
    pub async fn create_worker<CP: ClientPoolTrait + Send + Sync + 'static>(
        &self,
        worker_id: String,
        tenant_ids: Vec<Uuid>,
        client_pool: Arc<CP>,
    ) -> Result<()> {
        let worker = Arc::new(MonitorWorker::new(
            worker_id.clone(),
            self.db.clone(),
            self.cache.clone(),
            self.config.clone(),
        ));

        worker.assign_tenants(tenant_ids).await;

        // Add to pool
        self.workers
            .write()
            .await
            .insert(worker_id.clone(), worker.clone());

        // Start worker in background
        let worker_clone = worker.clone();
        tokio::spawn(async move {
            if let Err(e) = worker_clone.start(client_pool).await {
                error!("Worker failed to start: {}", e);
            }
        });

        Ok(())
    }

    /// Get worker status
    pub async fn get_worker_status(&self, worker_id: &str) -> Option<WorkerStatus> {
        let workers = self.workers.read().await;
        if let Some(worker) = workers.get(worker_id) {
            Some(worker.status.read().await.clone())
        } else {
            None
        }
    }

    /// List all workers
    pub async fn list_workers(&self) -> Vec<(String, WorkerStatus, usize)> {
        let workers = self.workers.read().await;
        let mut result = Vec::new();

        for (id, worker) in workers.iter() {
            let status = worker.status.read().await.clone();
            let tenant_count = worker.assigned_tenants.read().await.len();
            result.push((id.clone(), status, tenant_count));
        }

        result
    }

    /// Reassign tenants to a worker
    pub async fn reassign_tenants(&self, worker_id: &str, tenant_ids: Vec<Uuid>) -> Result<()> {
        let workers = self.workers.read().await;
        if let Some(worker) = workers.get(worker_id) {
            worker.assign_tenants(tenant_ids).await;
            Ok(())
        } else {
            anyhow::bail!("Worker {} not found", worker_id)
        }
    }

    /// Stop and remove a worker
    pub async fn remove_worker(&self, worker_id: &str) -> Result<()> {
        let mut workers = self.workers.write().await;
        if let Some(worker) = workers.remove(worker_id) {
            *worker.status.write().await = WorkerStatus::Stopping;
            Ok(())
        } else {
            anyhow::bail!("Worker {} not found", worker_id)
        }
    }
}
