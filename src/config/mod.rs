//! Configuration module for the orchestrator
//!
//! This module provides all configuration structures for the orchestrator,
//! following a similar pattern to OpenZeppelin Monitor's configuration layout.

// Sub-modules for each configuration type
pub mod api;
pub mod block_cache;
pub mod block_watcher;
pub mod error;
pub mod load_balancer;
pub mod orchestrator;
pub mod service_mode;
pub mod worker;

// Re-export main types
pub use api::ApiConfig;
pub use block_cache::BlockCacheConfig;
pub use block_watcher::SharedBlockWatcherConfig;
pub use error::ConfigError;
pub use load_balancer::{LoadBalancerConfig, LoadBalancingStrategy};
pub use orchestrator::OrchestratorConfig;
pub use service_mode::ServiceMode;
pub use worker::WorkerConfig;
