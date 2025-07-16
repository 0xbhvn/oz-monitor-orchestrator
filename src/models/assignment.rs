//! Assignment models for tenant-to-worker mapping

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Tenant assignment to a worker
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantAssignment {
    /// Tenant identifier
    pub tenant_id: Uuid,

    /// Assigned worker identifier
    pub worker_id: String,

    /// Assignment timestamp
    pub assigned_at: DateTime<Utc>,

    /// Assignment version (for tracking reassignments)
    pub version: u32,

    /// Assignment reason
    pub reason: AssignmentReason,
}

/// Reason for tenant assignment
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssignmentReason {
    /// Initial assignment
    Initial,

    /// Rebalancing due to load
    LoadRebalance,

    /// Worker failure
    WorkerFailure,

    /// Manual reassignment
    Manual,

    /// Scaling event (up or down)
    Scaling,

    /// Priority-based reassignment
    PriorityChange,
}

/// Worker assignment summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerAssignment {
    /// Worker identifier
    pub worker_id: String,

    /// List of assigned tenant IDs
    pub tenant_ids: Vec<Uuid>,

    /// Total load score (0.0 to 1.0)
    pub load_score: f64,

    /// Number of high-priority tenants
    pub high_priority_count: usize,

    /// Last updated timestamp
    pub updated_at: DateTime<Utc>,
}

impl TenantAssignment {
    /// Create a new tenant assignment
    pub fn new(tenant_id: Uuid, worker_id: String, reason: AssignmentReason) -> Self {
        Self {
            tenant_id,
            worker_id,
            assigned_at: Utc::now(),
            version: 1,
            reason,
        }
    }

    /// Create a reassignment (increments version)
    pub fn reassign(&self, new_worker_id: String, reason: AssignmentReason) -> Self {
        Self {
            tenant_id: self.tenant_id,
            worker_id: new_worker_id,
            assigned_at: Utc::now(),
            version: self.version + 1,
            reason,
        }
    }
}

impl WorkerAssignment {
    /// Create a new worker assignment
    pub fn new(worker_id: String) -> Self {
        Self {
            worker_id,
            tenant_ids: Vec::new(),
            load_score: 0.0,
            high_priority_count: 0,
            updated_at: Utc::now(),
        }
    }

    /// Add a tenant to this worker
    pub fn add_tenant(&mut self, tenant_id: Uuid) {
        if !self.tenant_ids.contains(&tenant_id) {
            self.tenant_ids.push(tenant_id);
            self.updated_at = Utc::now();
        }
    }

    /// Remove a tenant from this worker
    pub fn remove_tenant(&mut self, tenant_id: &Uuid) -> bool {
        let initial_len = self.tenant_ids.len();
        self.tenant_ids.retain(|id| id != tenant_id);
        let removed = self.tenant_ids.len() < initial_len;
        if removed {
            self.updated_at = Utc::now();
        }
        removed
    }

    /// Get tenant count
    pub fn tenant_count(&self) -> usize {
        self.tenant_ids.len()
    }

    /// Check if worker has capacity (assuming max 50 tenants)
    pub fn has_capacity(&self, max_tenants: usize) -> bool {
        self.tenant_ids.len() < max_tenants
    }
}
