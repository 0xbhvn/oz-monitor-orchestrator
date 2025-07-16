//! Load balancer configuration

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Load balancing strategy
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
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

impl Default for LoadBalancingStrategy {
    fn default() -> Self {
        LoadBalancingStrategy::ConsistentHashing
    }
}

/// Load balancer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadBalancerConfig {
    /// Load balancing strategy
    #[serde(default)]
    pub strategy: LoadBalancingStrategy,

    /// Maximum tenants per worker
    pub max_tenants_per_worker: usize,

    /// Rebalance threshold (0.0 to 1.0)
    pub rebalance_threshold: f64,

    /// Minimum interval between rebalances
    #[serde(with = "humantime_serde")]
    pub min_rebalance_interval: Duration,
}

impl Default for LoadBalancerConfig {
    fn default() -> Self {
        Self {
            strategy: LoadBalancingStrategy::ConsistentHashing,
            max_tenants_per_worker: 50,
            rebalance_threshold: 0.2, // 20% imbalance triggers rebalance
            min_rebalance_interval: Duration::from_secs(300), // 5 minutes
        }
    }
}

impl LoadBalancerConfig {
    /// Validate load balancer configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.max_tenants_per_worker == 0 {
            return Err("max_tenants_per_worker must be greater than 0".to_string());
        }

        if self.rebalance_threshold < 0.0 || self.rebalance_threshold > 1.0 {
            return Err("rebalance_threshold must be between 0.0 and 1.0".to_string());
        }

        if self.min_rebalance_interval.as_secs() < 60 {
            return Err("min_rebalance_interval must be at least 60 seconds".to_string());
        }

        Ok(())
    }
}

// Re-export for backward compatibility with services
impl From<LoadBalancerConfig> for crate::services::load_balancer::LoadBalancerConfig {
    fn from(config: LoadBalancerConfig) -> Self {
        // Map our strategy enum to the service's enum
        let strategy = match config.strategy {
            LoadBalancingStrategy::RoundRobin => {
                crate::services::load_balancer::LoadBalancingStrategy::RoundRobin
            }
            LoadBalancingStrategy::LeastLoaded => {
                crate::services::load_balancer::LoadBalancingStrategy::LeastLoaded
            }
            LoadBalancingStrategy::ConsistentHashing => {
                crate::services::load_balancer::LoadBalancingStrategy::ConsistentHashing
            }
            LoadBalancingStrategy::ActivityBased => {
                crate::services::load_balancer::LoadBalancingStrategy::ActivityBased
            }
        };

        crate::services::load_balancer::LoadBalancerConfig {
            strategy,
            max_tenants_per_worker: config.max_tenants_per_worker,
            rebalance_threshold: config.rebalance_threshold,
            min_rebalance_interval: config.min_rebalance_interval,
        }
    }
}
