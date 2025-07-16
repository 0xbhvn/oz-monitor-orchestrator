//! Model validation error types

use thiserror::Error;
use uuid::Uuid;

/// Model-related errors
#[derive(Error, Debug)]
pub enum ModelError {
    /// Invalid tenant ID
    #[error("Invalid tenant ID: {0}")]
    InvalidTenantId(Uuid),

    /// Invalid worker ID
    #[error("Invalid worker ID: {0}")]
    InvalidWorkerId(String),

    /// Invalid metric value
    #[error("Invalid metric value: {field} = {value}")]
    InvalidMetric { field: String, value: String },

    /// Invalid priority
    #[error("Invalid priority value: {0}")]
    InvalidPriority(u8),

    /// Invalid status
    #[error("Invalid status: {0}")]
    InvalidStatus(String),

    /// Validation error
    #[error("Validation failed: {0}")]
    ValidationError(String),
}
