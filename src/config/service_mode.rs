//! Service mode configuration

use serde::{Deserialize, Serialize};

/// Service mode for the orchestrator
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum ServiceMode {
    /// Run as a monitor worker processing tenant configurations
    Worker,

    /// Run as a shared block watcher fetching blocks for all workers
    BlockWatcher,

    /// Run the management API server
    Api,

    /// Run all services (for development)
    All,
}

impl Default for ServiceMode {
    fn default() -> Self {
        ServiceMode::Worker
    }
}

impl std::fmt::Display for ServiceMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServiceMode::Worker => write!(f, "worker"),
            ServiceMode::BlockWatcher => write!(f, "block-watcher"),
            ServiceMode::Api => write!(f, "api"),
            ServiceMode::All => write!(f, "all"),
        }
    }
}

impl std::str::FromStr for ServiceMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "worker" => Ok(ServiceMode::Worker),
            "block-watcher" | "blockwatcher" => Ok(ServiceMode::BlockWatcher),
            "api" => Ok(ServiceMode::Api),
            "all" => Ok(ServiceMode::All),
            _ => Err(format!("Invalid service mode: {}", s)),
        }
    }
}
