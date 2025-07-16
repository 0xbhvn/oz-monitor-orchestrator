//! Repository error types

use thiserror::Error;
use uuid::Uuid;

/// Repository-related errors
#[derive(Error, Debug)]
pub enum RepositoryError {
    /// Database connection error
    #[error("Database connection error: {0}")]
    ConnectionError(String),

    /// Query execution error
    #[error("Query execution failed: {0}")]
    QueryError(String),

    /// Entity not found
    #[error("Entity not found: {entity_type} with id {id}")]
    NotFound { entity_type: String, id: String },

    /// Tenant not found
    #[error("Tenant not found: {0}")]
    TenantNotFound(Uuid),

    /// Serialization/deserialization error
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// Transaction error
    #[error("Transaction failed: {0}")]
    TransactionError(String),

    /// Constraint violation
    #[error("Constraint violation: {0}")]
    ConstraintViolation(String),
}

impl From<sqlx::Error> for RepositoryError {
    fn from(err: sqlx::Error) -> Self {
        match err {
            sqlx::Error::RowNotFound => RepositoryError::NotFound {
                entity_type: "Unknown".to_string(),
                id: "Unknown".to_string(),
            },
            sqlx::Error::Database(db_err) => {
                if db_err.is_unique_violation() {
                    RepositoryError::ConstraintViolation(db_err.to_string())
                } else {
                    RepositoryError::QueryError(db_err.to_string())
                }
            }
            _ => RepositoryError::QueryError(err.to_string()),
        }
    }
}

impl From<serde_json::Error> for RepositoryError {
    fn from(err: serde_json::Error) -> Self {
        RepositoryError::SerializationError(err.to_string())
    }
}
