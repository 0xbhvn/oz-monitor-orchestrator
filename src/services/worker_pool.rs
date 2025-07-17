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

// // Import OpenZeppelin Monitor types
// use openzeppelin_monitor::{
//     models::{BlockType, Monitor, Network},
//     services::blockchain::ClientPoolTrait,
// };

use crate::services::{
    block_cache::BlockCacheService,
    cached_client_pool::CachedClientPool,
    oz_monitor_integration::OzMonitorServices,
    shared_block_watcher::{BlockEvent, SharedBlockWatcher},
};

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
    _cache: Arc<BlockCacheService>,
    config: WorkerConfig,
    oz_services: Option<Arc<OzMonitorServices>>,
    client_pool: Option<Arc<CachedClientPool>>,
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
            _cache: cache,
            config,
            oz_services: None,
            client_pool: None,
        }
    }

    /// Assign tenants to this worker
    pub async fn assign_tenants(&self, tenant_ids: Vec<Uuid>) {
        let mut tenants = self.assigned_tenants.write().await;
        *tenants = tenant_ids;
        info!("Worker {} assigned {} tenants", self.id, tenants.len());
    }

    /// Start the worker
    #[instrument(skip(self, block_watcher, client_pool), fields(worker_id = %self.id))]
    pub async fn start(
        &mut self,
        block_watcher: Arc<SharedBlockWatcher>,
        client_pool: Arc<CachedClientPool>,
    ) -> Result<()> {
        *self.status.write().await = WorkerStatus::Running;
        info!("Starting worker {}", self.id);

        // Initialize OZ Monitor services for assigned tenants
        let tenant_ids = self.assigned_tenants.read().await.clone();
        if tenant_ids.is_empty() {
            warn!("Worker {} has no assigned tenants", self.id);
            return Ok(());
        }

        // Store client pool
        self.client_pool = Some(client_pool.clone());

        let oz_services =
            match OzMonitorServices::new(self.db.clone(), tenant_ids.clone(), client_pool).await {
                Ok(services) => Arc::new(services),
                Err(e) => {
                    error!("Failed to initialize OZ Monitor services: {}", e);
                    *self.status.write().await = WorkerStatus::Error(e.to_string());
                    return Err(e);
                }
            };

        self.oz_services = Some(oz_services.clone());

        // Subscribe to block events
        let block_receiver = block_watcher.subscribe();

        // Start background tasks
        let health_handle = self.start_health_check();
        let reload_handle = self.start_tenant_reload();
        let monitor_handle = self
            .start_monitoring_with_events(oz_services, block_receiver)
            .await?;

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

    /// Start monitoring task with block events
    async fn start_monitoring_with_events(
        &self,
        oz_services: Arc<OzMonitorServices>,
        mut block_receiver: tokio::sync::broadcast::Receiver<BlockEvent>,
    ) -> Result<tokio::task::JoinHandle<()>> {
        let tenants = self.assigned_tenants.clone();
        let worker_id = self.id.clone();
        let status = self.status.clone();

        let handle = tokio::spawn(async move {
            loop {
                // Wait for block events
                match block_receiver.recv().await {
                    Ok(block_event) => {
                        let tenant_ids = tenants.read().await.clone();
                        if tenant_ids.is_empty() {
                            continue;
                        }

                        info!(
                            "Worker {} processing {} blocks for network {} ({} tenants)",
                            worker_id,
                            block_event.blocks.len(),
                            block_event.network.slug,
                            tenant_ids.len()
                        );

                        // Process each block
                        for block in block_event.blocks {
                            match oz_services
                                .process_block(&block_event.network, block, &tenant_ids)
                                .await
                            {
                                Ok(results) => {
                                    let total_matches = results.len();

                                    if total_matches > 0 {
                                        info!(
                                            "Worker {} found {} matches on network {}",
                                            worker_id, total_matches, block_event.network.slug
                                        );
                                    }
                                }
                                Err(e) => {
                                    error!(
                                        "Worker {} failed to process block on network {}: {}",
                                        worker_id, block_event.network.slug, e
                                    );
                                    *status.write().await = WorkerStatus::Error(e.to_string());
                                }
                            }
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                        warn!("Worker {} lagged behind by {} messages", worker_id, skipped);
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        info!("Block event channel closed, stopping worker {}", worker_id);
                        break;
                    }
                }
            }
        });

        Ok(handle)
    }
}

/// Monitor worker pool manager
pub struct MonitorWorkerPool {
    workers: Arc<RwLock<HashMap<String, Arc<RwLock<MonitorWorker>>>>>,
    db: Arc<PgPool>,
    _cache: Arc<BlockCacheService>,
    config: WorkerConfig,
}

impl MonitorWorkerPool {
    pub fn new(db: Arc<PgPool>, cache: Arc<BlockCacheService>, config: WorkerConfig) -> Self {
        Self {
            workers: Arc::new(RwLock::new(HashMap::new())),
            db,
            _cache: cache,
            config,
        }
    }

    /// Create and start a new worker
    pub async fn create_worker(
        &self,
        worker_id: String,
        tenant_ids: Vec<Uuid>,
        block_watcher: Arc<SharedBlockWatcher>,
        client_pool: Arc<CachedClientPool>,
    ) -> Result<()> {
        let worker = MonitorWorker::new(
            worker_id.clone(),
            self.db.clone(),
            self._cache.clone(),
            self.config.clone(),
        );

        worker.assign_tenants(tenant_ids).await;

        // Add to pool
        let worker_arc = Arc::new(RwLock::new(worker));
        self.workers
            .write()
            .await
            .insert(worker_id.clone(), worker_arc.clone());

        // Start worker in background
        tokio::spawn(async move {
            let mut worker_lock = worker_arc.write().await;
            if let Err(e) = worker_lock.start(block_watcher, client_pool).await {
                error!("Worker failed to start: {}", e);
            }
        });

        Ok(())
    }

    /// Get worker status
    pub async fn get_worker_status(&self, worker_id: &str) -> Option<WorkerStatus> {
        let workers = self.workers.read().await;
        if let Some(worker) = workers.get(worker_id) {
            let worker_lock = worker.read().await;
            let status = worker_lock.status.read().await.clone();
            Some(status)
        } else {
            None
        }
    }

    /// List all workers
    pub async fn list_workers(&self) -> Vec<(String, WorkerStatus, usize)> {
        let workers = self.workers.read().await;
        let mut result = Vec::new();

        for (id, worker) in workers.iter() {
            let worker_lock = worker.read().await;
            let status = worker_lock.status.read().await.clone();
            let tenant_count = worker_lock.assigned_tenants.read().await.len();
            result.push((id.clone(), status, tenant_count));
        }

        result
    }

    /// Reassign tenants to a worker
    pub async fn reassign_tenants(&self, worker_id: &str, tenant_ids: Vec<Uuid>) -> Result<()> {
        let workers = self.workers.read().await;
        if let Some(worker) = workers.get(worker_id) {
            let worker_lock = worker.read().await;
            worker_lock.assign_tenants(tenant_ids.clone()).await;

            // Reload OZ Monitor services with new tenant list if worker is running
            if let Some(oz_services) = &worker_lock.oz_services {
                oz_services.reload_configurations(&tenant_ids).await?;
            }

            Ok(())
        } else {
            anyhow::bail!("Worker {} not found", worker_id)
        }
    }

    /// Stop and remove a worker
    pub async fn remove_worker(&self, worker_id: &str) -> Result<()> {
        let mut workers = self.workers.write().await;
        if let Some(worker) = workers.remove(worker_id) {
            let worker_lock = worker.write().await;
            *worker_lock.status.write().await = WorkerStatus::Stopping;
            Ok(())
        } else {
            anyhow::bail!("Worker {} not found", worker_id)
        }
    }
}
