//! Main orchestrator configuration

use config::{Config, ConfigError, File};
use serde::{Deserialize, Serialize};

use super::{
    ApiConfig, BlockCacheConfig, LoadBalancerConfig, ServiceMode, SharedBlockWatcherConfig,
    WorkerConfig,
};

/// Main orchestrator configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestratorConfig {
    /// Database connection URL
    pub database_url: String,

    /// Redis connection URL
    pub redis_url: String,

    /// Service mode (worker, block-watcher, api)
    #[serde(default = "default_service_mode")]
    pub service_mode: ServiceMode,

    /// Worker configuration
    #[serde(default)]
    pub worker: WorkerConfig,

    /// Block cache configuration
    #[serde(default)]
    pub block_cache: BlockCacheConfig,

    /// Load balancer configuration
    #[serde(default)]
    pub load_balancer: LoadBalancerConfig,

    /// Shared block watcher configuration
    #[serde(default)]
    pub block_watcher: SharedBlockWatcherConfig,

    /// API server configuration
    #[serde(default)]
    pub api: ApiConfig,
}

fn default_service_mode() -> ServiceMode {
    ServiceMode::Worker
}

impl OrchestratorConfig {
    /// Load configuration from file and environment
    pub fn load() -> Result<Self, ConfigError> {
        let config = Config::builder()
            // Start with default values
            .set_default("service_mode", "worker")?
            // Add config file if exists
            .add_source(File::with_name("/etc/oz-monitor/config.yaml").required(false))
            .add_source(File::with_name("config.yaml").required(false))
            // Override with environment variables
            .add_source(config::Environment::with_prefix("OZ_MONITOR"))
            .build()?;

        config.try_deserialize()
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.database_url.is_empty() {
            return Err("Database URL is required".to_string());
        }

        if self.redis_url.is_empty() {
            return Err("Redis URL is required".to_string());
        }

        // Delegate validation to sub-configs
        self.worker.validate()?;
        self.load_balancer.validate()?;
        self.block_watcher.validate()?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = OrchestratorConfig {
            database_url: "postgresql://test".to_string(),
            redis_url: "redis://test".to_string(),
            service_mode: ServiceMode::Worker,
            worker: Default::default(),
            block_cache: Default::default(),
            load_balancer: Default::default(),
            block_watcher: Default::default(),
            api: Default::default(),
        };

        assert_eq!(config.validate(), Ok(()));
    }

    #[test]
    fn test_invalid_config() {
        let config = OrchestratorConfig {
            database_url: "".to_string(),
            redis_url: "redis://test".to_string(),
            service_mode: ServiceMode::Worker,
            worker: Default::default(),
            block_cache: Default::default(),
            load_balancer: Default::default(),
            block_watcher: Default::default(),
            api: Default::default(),
        };

        assert!(config.validate().is_err());
    }
}
