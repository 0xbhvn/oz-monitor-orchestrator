//! Block cache configuration

use serde::{Deserialize, Serialize};

/// Configuration for the block cache service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockCacheConfig {
    /// TTL for cached blocks in seconds
    pub block_ttl: u64,

    /// TTL for latest block number in seconds
    pub latest_block_ttl: u64,

    /// Redis key prefix for cache entries
    pub key_prefix: String,
}

impl Default for BlockCacheConfig {
    fn default() -> Self {
        Self {
            block_ttl: 60,       // 1 minute for blocks
            latest_block_ttl: 5, // 5 seconds for latest block
            key_prefix: "oz_cache".to_string(),
        }
    }
}

impl BlockCacheConfig {
    /// Validate block cache configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.block_ttl == 0 {
            return Err("block_ttl must be greater than 0".to_string());
        }

        if self.latest_block_ttl == 0 {
            return Err("latest_block_ttl must be greater than 0".to_string());
        }

        if self.key_prefix.is_empty() {
            return Err("key_prefix cannot be empty".to_string());
        }

        Ok(())
    }
}

// Re-export for backward compatibility with services
impl From<BlockCacheConfig> for crate::services::block_cache::BlockCacheConfig {
    fn from(config: BlockCacheConfig) -> Self {
        crate::services::block_cache::BlockCacheConfig {
            block_ttl: config.block_ttl,
            latest_block_ttl: config.latest_block_ttl,
            key_prefix: config.key_prefix,
        }
    }
}
