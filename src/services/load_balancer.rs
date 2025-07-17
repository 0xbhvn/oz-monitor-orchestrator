//! Load Balancer Service
//!
//! Distributes tenants across workers based on resource usage and activity.

use anyhow::Result;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, instrument};
use uuid::Uuid;

// Import models from our models module
use crate::models::{AssignmentReason, TenantAssignment, TenantMetrics, WorkerMetrics};

/// Load balancing strategy
#[derive(Debug, Clone)]
pub enum LoadBalancingStrategy {
    /// Round-robin distribution
    RoundRobin,
    /// Least loaded worker first
    LeastLoaded,
    /// Consistent hashing with tenant affinity
    ConsistentHashing,
    /// Activity-based distribution
    ActivityBased,
}

/// Load balancer configuration
#[derive(Debug, Clone)]
pub struct LoadBalancerConfig {
    pub strategy: LoadBalancingStrategy,
    pub max_tenants_per_worker: usize,
    pub rebalance_threshold: f64,
    pub min_rebalance_interval: std::time::Duration,
}

impl Default for LoadBalancerConfig {
    fn default() -> Self {
        Self {
            strategy: LoadBalancingStrategy::ConsistentHashing,
            max_tenants_per_worker: 50,
            rebalance_threshold: 0.2, // 20% imbalance triggers rebalance
            min_rebalance_interval: std::time::Duration::from_secs(300), // 5 minutes
        }
    }
}

/// Load balancer service
pub struct LoadBalancer {
    assignments: Arc<RwLock<HashMap<Uuid, TenantAssignment>>>,
    worker_loads: Arc<RwLock<HashMap<String, WorkerMetrics>>>,
    tenant_metrics: Arc<RwLock<HashMap<Uuid, TenantMetrics>>>,
    /// Mapping from tenant to worker for consistent hashing
    tenant_worker_map: Arc<RwLock<HashMap<String, String>>>,
    config: LoadBalancerConfig,
    last_rebalance: Arc<RwLock<chrono::DateTime<chrono::Utc>>>,
}

impl LoadBalancer {
    pub fn new(config: LoadBalancerConfig) -> Self {
        Self {
            assignments: Arc::new(RwLock::new(HashMap::new())),
            worker_loads: Arc::new(RwLock::new(HashMap::new())),
            tenant_metrics: Arc::new(RwLock::new(HashMap::new())),
            tenant_worker_map: Arc::new(RwLock::new(HashMap::new())),
            config,
            last_rebalance: Arc::new(RwLock::new(chrono::Utc::now())),
        }
    }

    /// Add a new worker
    pub async fn add_worker(&self, worker_id: String) -> Result<()> {
        let mut worker_loads = self.worker_loads.write().await;
        worker_loads.insert(
            worker_id.clone(),
            WorkerMetrics {
                worker_id: worker_id.clone(),
                tenant_count: 0,
                cpu_usage: 0.0,
                memory_usage: 0.0,
                rpc_rate: 0.0,
                avg_processing_time_ms: 0.0,
                errors_last_hour: 0,
                uptime_seconds: 0,
                collected_at: chrono::Utc::now(),
            },
        );

        // Update tenant-worker map will happen during assignment

        info!("Added worker {} to load balancer", worker_id);
        Ok(())
    }

    /// Remove a worker and reassign its tenants
    pub async fn remove_worker(&self, worker_id: &str) -> Result<Vec<Uuid>> {
        let mut worker_loads = self.worker_loads.write().await;
        worker_loads.remove(worker_id);

        // Remove from tenant-worker map
        let mut tenant_worker_map = self.tenant_worker_map.write().await;
        tenant_worker_map.retain(|_, v| v != worker_id);

        // Find tenants assigned to this worker
        let mut assignments = self.assignments.write().await;
        let mut reassigned_tenants = Vec::new();

        assignments.retain(|tenant_id, assignment| {
            if assignment.worker_id == worker_id {
                reassigned_tenants.push(*tenant_id);
                false
            } else {
                true
            }
        });

        info!(
            "Removed worker {} from load balancer, {} tenants need reassignment",
            worker_id,
            reassigned_tenants.len()
        );

        Ok(reassigned_tenants)
    }

    /// Update worker load metrics
    pub async fn update_worker_load(&self, metrics: WorkerMetrics) -> Result<()> {
        let mut worker_loads = self.worker_loads.write().await;
        worker_loads.insert(metrics.worker_id.clone(), metrics);
        Ok(())
    }

    /// Update tenant metrics
    pub async fn update_tenant_metrics(&self, metrics: TenantMetrics) -> Result<()> {
        let mut tenant_metrics = self.tenant_metrics.write().await;
        tenant_metrics.insert(metrics.tenant_id, metrics);
        Ok(())
    }

    /// Assign a tenant to a worker
    #[instrument(skip(self))]
    pub async fn assign_tenant(&self, tenant_id: Uuid) -> Result<String> {
        let worker_id = match self.config.strategy {
            LoadBalancingStrategy::RoundRobin => self.round_robin_assignment().await?,
            LoadBalancingStrategy::LeastLoaded => self.least_loaded_assignment().await?,
            LoadBalancingStrategy::ConsistentHashing => {
                self.consistent_hash_assignment(tenant_id).await?
            }
            LoadBalancingStrategy::ActivityBased => {
                self.activity_based_assignment(tenant_id).await?
            }
        };

        // Record assignment
        let mut assignments = self.assignments.write().await;
        let reason = match self.config.strategy {
            LoadBalancingStrategy::RoundRobin => AssignmentReason::Initial,
            LoadBalancingStrategy::LeastLoaded => AssignmentReason::LoadRebalance,
            LoadBalancingStrategy::ConsistentHashing => AssignmentReason::Initial,
            LoadBalancingStrategy::ActivityBased => AssignmentReason::LoadRebalance,
        };
        assignments.insert(
            tenant_id,
            TenantAssignment::new(tenant_id, worker_id.clone(), reason),
        );

        // Update worker load
        let mut worker_loads = self.worker_loads.write().await;
        if let Some(load) = worker_loads.get_mut(&worker_id) {
            load.tenant_count += 1;
        }

        info!("Assigned tenant {} to worker {}", tenant_id, worker_id);
        Ok(worker_id)
    }

    /// Get worker for a tenant
    pub async fn get_worker_for_tenant(&self, tenant_id: Uuid) -> Option<String> {
        let assignments = self.assignments.read().await;
        assignments.get(&tenant_id).map(|a| a.worker_id.clone())
    }

    /// Check if rebalancing is needed
    pub async fn needs_rebalancing(&self) -> bool {
        // Check minimum interval
        let last_rebalance = *self.last_rebalance.read().await;
        if chrono::Utc::now() - last_rebalance
            < chrono::Duration::from_std(self.config.min_rebalance_interval).unwrap()
        {
            return false;
        }

        // Check load imbalance
        let worker_loads = self.worker_loads.read().await;
        if worker_loads.len() < 2 {
            return false;
        }

        let loads: Vec<f64> = worker_loads
            .values()
            .map(|l| l.tenant_count as f64)
            .collect();

        let avg_load = loads.iter().sum::<f64>() / loads.len() as f64;
        let max_load = loads.iter().fold(0.0f64, |a, &b| a.max(b));
        let min_load = loads.iter().fold(f64::MAX, |a, &b| a.min(b));

        let imbalance = (max_load - min_load) / avg_load;
        imbalance > self.config.rebalance_threshold
    }

    /// Rebalance tenants across workers
    #[instrument(skip(self))]
    pub async fn rebalance(&self) -> Result<HashMap<String, Vec<Uuid>>> {
        info!("Starting tenant rebalancing");

        let tenant_metrics = self.tenant_metrics.read().await;
        let worker_loads = self.worker_loads.read().await;

        if worker_loads.is_empty() {
            return Ok(HashMap::new());
        }

        // Group tenants by activity level
        let mut high_activity = Vec::new();
        let mut medium_activity = Vec::new();
        let mut low_activity = Vec::new();

        for (tenant_id, metrics) in tenant_metrics.iter() {
            let activity_score = metrics.activity_score();
            if activity_score > 0.7 {
                high_activity.push((*tenant_id, activity_score));
            } else if activity_score > 0.3 {
                medium_activity.push((*tenant_id, activity_score));
            } else {
                low_activity.push((*tenant_id, activity_score));
            }
        }

        // Sort by activity score
        high_activity.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        medium_activity.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        low_activity.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        // Create new assignments
        let mut new_assignments: HashMap<String, Vec<Uuid>> = HashMap::new();
        let worker_ids: Vec<String> = worker_loads.keys().cloned().collect();
        let mut worker_scores: HashMap<String, f64> = HashMap::new();

        for worker_id in &worker_ids {
            new_assignments.insert(worker_id.clone(), Vec::new());
            worker_scores.insert(worker_id.clone(), 0.0);
        }

        // Assign high activity tenants first, distributing them evenly
        for (tenant_id, score) in high_activity {
            let worker_id = worker_scores
                .iter()
                .min_by_key(|(_, &score)| (score * 1000.0) as i64)
                .map(|(id, _)| id.clone())
                .unwrap();

            new_assignments.get_mut(&worker_id).unwrap().push(tenant_id);
            *worker_scores.get_mut(&worker_id).unwrap() += score;
        }

        // Then medium activity
        for (tenant_id, score) in medium_activity {
            let worker_id = worker_scores
                .iter()
                .min_by_key(|(_, &score)| (score * 1000.0) as i64)
                .map(|(id, _)| id.clone())
                .unwrap();

            new_assignments.get_mut(&worker_id).unwrap().push(tenant_id);
            *worker_scores.get_mut(&worker_id).unwrap() += score;
        }

        // Finally low activity
        for (tenant_id, score) in low_activity {
            let worker_id = worker_scores
                .iter()
                .min_by_key(|(_, &score)| (score * 1000.0) as i64)
                .map(|(id, _)| id.clone())
                .unwrap();

            new_assignments.get_mut(&worker_id).unwrap().push(tenant_id);
            *worker_scores.get_mut(&worker_id).unwrap() += score;
        }

        // Update assignments
        let mut assignments = self.assignments.write().await;
        assignments.clear();

        for (worker_id, tenant_ids) in &new_assignments {
            for tenant_id in tenant_ids {
                assignments.insert(
                    *tenant_id,
                    TenantAssignment::new(
                        *tenant_id,
                        worker_id.clone(),
                        AssignmentReason::LoadRebalance,
                    ),
                );
            }
        }

        *self.last_rebalance.write().await = chrono::Utc::now();

        info!(
            "Rebalancing complete. New distribution: {:?}",
            new_assignments
                .iter()
                .map(|(k, v)| (k, v.len()))
                .collect::<Vec<_>>()
        );

        Ok(new_assignments)
    }

    /// Round-robin assignment
    async fn round_robin_assignment(&self) -> Result<String> {
        let worker_loads = self.worker_loads.read().await;

        worker_loads
            .iter()
            .min_by_key(|(_, load)| load.tenant_count)
            .map(|(id, _)| id.clone())
            .ok_or_else(|| anyhow::anyhow!("No workers available"))
    }

    /// Least loaded assignment
    async fn least_loaded_assignment(&self) -> Result<String> {
        let worker_loads = self.worker_loads.read().await;

        worker_loads
            .iter()
            .min_by_key(|(_, load)| {
                (load.cpu_usage * 100.0) as i32
                    + (load.memory_usage * 100.0) as i32
                    + load.tenant_count as i32
            })
            .map(|(id, _)| id.clone())
            .ok_or_else(|| anyhow::anyhow!("No workers available"))
    }

    /// Consistent hash assignment
    async fn consistent_hash_assignment(&self, tenant_id: Uuid) -> Result<String> {
        let tenant_worker_map = self.tenant_worker_map.read().await;
        let worker_loads = self.worker_loads.read().await;

        // Check if tenant already has an assigned worker
        if let Some(worker_id) = tenant_worker_map.get(&tenant_id.to_string()) {
            if worker_loads.contains_key(worker_id) {
                return Ok(worker_id.clone());
            }
        }

        // If not, use simple hash-based assignment
        let workers: Vec<String> = worker_loads.keys().cloned().collect();
        if workers.is_empty() {
            return Err(anyhow::anyhow!("No workers available"));
        }

        // Hash the tenant ID to select a worker
        let mut hasher = DefaultHasher::new();
        tenant_id.to_string().hash(&mut hasher);
        let hash = hasher.finish();
        let index = (hash as usize) % workers.len();

        Ok(workers[index].clone())
    }

    /// Activity-based assignment
    async fn activity_based_assignment(&self, tenant_id: Uuid) -> Result<String> {
        let tenant_metrics = self.tenant_metrics.read().await;
        let _worker_loads = self.worker_loads.read().await;

        // Default to least loaded if no metrics
        if let Some(metrics) = tenant_metrics.get(&tenant_id) {
            let activity_score = metrics.activity_score();

            // High activity tenants go to least loaded workers
            if activity_score > 0.7 {
                return self.least_loaded_assignment().await;
            }
        }

        // Low activity tenants use consistent hashing for affinity
        self.consistent_hash_assignment(tenant_id).await
    }

    /// Get all tenant assignments for a specific worker
    pub async fn get_worker_assignments(&self, worker_id: &str) -> Result<Vec<Uuid>> {
        let assignments = self.assignments.read().await;

        let tenant_ids: Vec<Uuid> = assignments
            .iter()
            .filter_map(|(tenant_id, assignment)| {
                if assignment.worker_id == worker_id {
                    Some(*tenant_id)
                } else {
                    None
                }
            })
            .collect();

        Ok(tenant_ids)
    }
}
