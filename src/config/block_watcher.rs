//! Shared block watcher configuration

use serde::{Deserialize, Serialize};

/// Shared block watcher configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedBlockWatcherConfig {
    /// Channel buffer size for block events
    pub channel_buffer_size: usize,

    /// Maximum blocks to fetch per iteration
    pub max_blocks_per_fetch: u64,

    /// Block fetch retry attempts
    pub retry_attempts: u32,

    /// Retry delay in milliseconds
    pub retry_delay_ms: u64,
}

impl Default for SharedBlockWatcherConfig {
    fn default() -> Self {
        Self {
            channel_buffer_size: 1000,
            max_blocks_per_fetch: 100,
            retry_attempts: 3,
            retry_delay_ms: 1000,
        }
    }
}

impl SharedBlockWatcherConfig {
    /// Validate block watcher configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.channel_buffer_size == 0 {
            return Err("channel_buffer_size must be greater than 0".to_string());
        }

        if self.max_blocks_per_fetch == 0 {
            return Err("max_blocks_per_fetch must be greater than 0".to_string());
        }

        if self.retry_attempts == 0 {
            return Err("retry_attempts must be greater than 0".to_string());
        }

        if self.retry_delay_ms == 0 {
            return Err("retry_delay_ms must be greater than 0".to_string());
        }

        Ok(())
    }
}

// Re-export for backward compatibility with services
impl From<SharedBlockWatcherConfig>
    for crate::services::shared_block_watcher::SharedBlockWatcherConfig
{
    fn from(config: SharedBlockWatcherConfig) -> Self {
        crate::services::shared_block_watcher::SharedBlockWatcherConfig {
            channel_buffer_size: config.channel_buffer_size,
            max_blocks_per_fetch: config.max_blocks_per_fetch,
            retry_attempts: config.retry_attempts,
            retry_delay_ms: config.retry_delay_ms,
        }
    }
}
