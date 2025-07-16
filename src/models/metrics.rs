//! Metrics models

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Tenant activity metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantMetrics {
    /// Tenant identifier
    pub tenant_id: Uuid,

    /// Number of active monitors
    pub monitors_count: usize,

    /// Average RPC calls per minute
    pub avg_rpc_calls_per_minute: f64,

    /// Average filter complexity score
    pub avg_filter_complexity: f64,

    /// Total matches in the last hour
    pub total_matches_last_hour: usize,

    /// Total notifications sent in the last hour
    pub notifications_sent_last_hour: usize,

    /// Last activity timestamp
    pub last_active: DateTime<Utc>,

    /// Metrics collection timestamp
    pub collected_at: DateTime<Utc>,
}

/// Worker performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerMetrics {
    /// Worker identifier
    pub worker_id: String,

    /// Number of assigned tenants
    pub tenant_count: usize,

    /// CPU usage percentage (0-100)
    pub cpu_usage: f64,

    /// Memory usage percentage (0-100)
    pub memory_usage: f64,

    /// RPC calls per second
    pub rpc_rate: f64,

    /// Average block processing time in milliseconds
    pub avg_processing_time_ms: f64,

    /// Number of errors in the last hour
    pub errors_last_hour: usize,

    /// Worker uptime in seconds
    pub uptime_seconds: u64,

    /// Metrics collection timestamp
    pub collected_at: DateTime<Utc>,
}

/// System-wide metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    /// Total active workers
    pub active_workers: usize,

    /// Total active tenants
    pub active_tenants: usize,

    /// Total monitors being processed
    pub total_monitors: usize,

    /// System-wide RPC rate
    pub total_rpc_rate: f64,

    /// Cache hit rate (0-1)
    pub cache_hit_rate: f64,

    /// Average block lag per network
    pub avg_block_lag: f64,

    /// Total matches in the last hour
    pub total_matches_last_hour: usize,

    /// System health score (0-100)
    pub health_score: f64,

    /// Metrics collection timestamp
    pub collected_at: DateTime<Utc>,
}

impl TenantMetrics {
    /// Calculate activity score (0.0 to 1.0)
    pub fn activity_score(&self) -> f64 {
        let rpc_score = (self.avg_rpc_calls_per_minute / 100.0).min(1.0);
        let complexity_score = (self.avg_filter_complexity / 10.0).min(1.0);
        let matches_score = (self.total_matches_last_hour as f64 / 1000.0).min(1.0);

        // Weighted average
        (rpc_score * 0.4 + complexity_score * 0.3 + matches_score * 0.3).min(1.0)
    }
}

impl WorkerMetrics {
    /// Calculate worker load score (0.0 to 1.0)
    pub fn load_score(&self) -> f64 {
        let cpu_score = self.cpu_usage / 100.0;
        let memory_score = self.memory_usage / 100.0;
        let tenant_score = (self.tenant_count as f64 / 50.0).min(1.0); // Assuming 50 is max

        // Weighted average
        (cpu_score * 0.4 + memory_score * 0.4 + tenant_score * 0.2).min(1.0)
    }

    /// Check if worker is healthy
    pub fn is_healthy(&self) -> bool {
        self.cpu_usage < 90.0 && self.memory_usage < 90.0 && self.errors_last_hour < 10
    }
}

impl SystemMetrics {
    /// Check if system is healthy
    pub fn is_healthy(&self) -> bool {
        self.health_score >= 70.0
    }

    /// Calculate health score based on various metrics
    pub fn calculate_health_score(&mut self) {
        let mut score: f64 = 100.0;

        // Deduct for high block lag
        if self.avg_block_lag > 100.0 {
            score -= 20.0;
        } else if self.avg_block_lag > 50.0 {
            score -= 10.0;
        }

        // Deduct for low cache hit rate
        if self.cache_hit_rate < 0.5 {
            score -= 20.0;
        } else if self.cache_hit_rate < 0.7 {
            score -= 10.0;
        }

        // Deduct if too few workers for tenants
        let tenant_per_worker = self.active_tenants as f64 / self.active_workers.max(1) as f64;
        if tenant_per_worker > 60.0 {
            score -= 15.0;
        }

        self.health_score = score.max(0.0);
    }
}
