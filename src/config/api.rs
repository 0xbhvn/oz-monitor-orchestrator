//! API server configuration

use serde::{Deserialize, Serialize};

/// API server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    /// Host address to bind to
    pub host: String,

    /// Port number to listen on
    pub port: u16,

    /// Enable CORS
    #[serde(default = "default_cors")]
    pub cors_enabled: bool,

    /// API rate limit (requests per minute)
    #[serde(default = "default_rate_limit")]
    pub rate_limit: u32,
}

fn default_cors() -> bool {
    true
}

fn default_rate_limit() -> u32 {
    100
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 3000,
            cors_enabled: true,
            rate_limit: 100,
        }
    }
}

impl ApiConfig {
    /// Validate API configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.host.is_empty() {
            return Err("host cannot be empty".to_string());
        }

        if self.port == 0 {
            return Err("port must be greater than 0".to_string());
        }

        if self.rate_limit == 0 {
            return Err("rate_limit must be greater than 0".to_string());
        }

        Ok(())
    }

    /// Get the socket address for binding
    pub fn socket_addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}
