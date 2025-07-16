//! Worker configuration

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Worker configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerConfig {
    /// Maximum number of tenants per worker
    pub max_tenants_per_worker: usize,

    /// Worker health check interval
    #[serde(with = "humantime_serde")]
    pub health_check_interval: Duration,

    /// Tenant configuration reload interval
    #[serde(with = "humantime_serde")]
    pub tenant_reload_interval: Duration,
}

impl Default for WorkerConfig {
    fn default() -> Self {
        Self {
            max_tenants_per_worker: 50,
            health_check_interval: Duration::from_secs(30),
            tenant_reload_interval: Duration::from_secs(300), // 5 minutes
        }
    }
}

impl WorkerConfig {
    /// Validate worker configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.max_tenants_per_worker == 0 {
            return Err("max_tenants_per_worker must be greater than 0".to_string());
        }

        if self.health_check_interval.as_secs() < 5 {
            return Err("health_check_interval must be at least 5 seconds".to_string());
        }

        if self.tenant_reload_interval.as_secs() < 30 {
            return Err("tenant_reload_interval must be at least 30 seconds".to_string());
        }

        Ok(())
    }
}

// Re-export for backward compatibility with services
impl From<WorkerConfig> for crate::services::worker_pool::WorkerConfig {
    fn from(config: WorkerConfig) -> Self {
        crate::services::worker_pool::WorkerConfig {
            max_tenants_per_worker: config.max_tenants_per_worker,
            health_check_interval: config.health_check_interval,
            tenant_reload_interval: config.tenant_reload_interval,
        }
    }
}
