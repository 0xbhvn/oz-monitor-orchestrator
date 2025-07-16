//! Configuration error types

use thiserror::Error;

/// Configuration-related errors
#[derive(Error, Debug)]
pub enum ConfigError {
    /// Configuration file not found
    #[error("Configuration file not found: {0}")]
    FileNotFound(String),

    /// Invalid configuration value
    #[error("Invalid configuration: {0}")]
    InvalidValue(String),

    /// Missing required configuration
    #[error("Missing required configuration: {0}")]
    MissingRequired(String),

    /// Configuration parsing error
    #[error("Failed to parse configuration: {0}")]
    ParseError(String),

    /// Environment variable error
    #[error("Environment variable error: {0}")]
    EnvError(String),

    /// Validation error
    #[error("Configuration validation failed: {0}")]
    ValidationError(String),
}

impl From<config::ConfigError> for ConfigError {
    fn from(err: config::ConfigError) -> Self {
        ConfigError::ParseError(err.to_string())
    }
}

impl From<std::env::VarError> for ConfigError {
    fn from(err: std::env::VarError) -> Self {
        ConfigError::EnvError(err.to_string())
    }
}
