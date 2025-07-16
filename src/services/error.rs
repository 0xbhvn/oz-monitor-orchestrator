//! Service layer error types

use thiserror::Error;
use uuid::Uuid;

use crate::config::ConfigError;
use crate::repositories::RepositoryError;

/// Service-related errors
#[derive(Error, Debug)]
pub enum ServiceError {
    /// Repository error
    #[error("Repository error: {0}")]
    Repository(#[from] RepositoryError),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Configuration(#[from] ConfigError),

    /// Worker not found
    #[error("Worker not found: {0}")]
    WorkerNotFound(String),

    /// Tenant not found
    #[error("Tenant not found: {0}")]
    TenantNotFound(Uuid),

    /// Resource limit exceeded
    #[error("Resource limit exceeded: {0}")]
    ResourceLimitExceeded(String),

    /// Service unavailable
    #[error("Service unavailable: {0}")]
    ServiceUnavailable(String),

    /// Invalid state
    #[error("Invalid state: {0}")]
    InvalidState(String),

    /// Communication error
    #[error("Communication error: {0}")]
    CommunicationError(String),

    /// Cache error
    #[error("Cache error: {0}")]
    CacheError(String),

    /// Block processing error
    #[error("Block processing error: {0}")]
    BlockProcessingError(String),

    /// Load balancing error
    #[error("Load balancing error: {0}")]
    LoadBalancingError(String),
}

impl From<redis::RedisError> for ServiceError {
    fn from(err: redis::RedisError) -> Self {
        ServiceError::CacheError(err.to_string())
    }
}

impl From<anyhow::Error> for ServiceError {
    fn from(err: anyhow::Error) -> Self {
        ServiceError::ServiceUnavailable(err.to_string())
    }
}
