//! Block Cache Service
//!
//! Provides a caching layer for blockchain RPC calls to prevent duplicate
//! requests across multiple monitor instances.

use anyhow::Result;
use async_trait::async_trait;
use redis::{AsyncCommands, Client as RedisClient};
use std::sync::Arc;
use tracing::{debug, instrument};

// Import OpenZeppelin Monitor types
use openzeppelin_monitor::{
    models::{BlockChainType, BlockType, Network},
    services::blockchain::BlockChainClient,
};

/// Configuration for the block cache
#[derive(Debug, Clone)]
pub struct BlockCacheConfig {
    /// TTL for cached blocks in seconds
    pub block_ttl: u64,
    /// TTL for latest block number in seconds
    pub latest_block_ttl: u64,
    /// Redis key prefix
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

/// Block cache service for sharing blocks across monitor instances
pub struct BlockCacheService {
    redis: Arc<RedisClient>,
    config: BlockCacheConfig,
}

impl BlockCacheService {
    pub async fn new(redis_url: &str, config: BlockCacheConfig) -> Result<Self> {
        let redis = RedisClient::open(redis_url)?;

        // Test connection
        let mut conn = redis.get_multiplexed_async_connection().await?;
        redis::cmd("PING").query_async::<()>(&mut conn).await?;

        Ok(Self {
            redis: Arc::new(redis),
            config,
        })
    }

    /// Get cached blocks or None if not found
    async fn get_cached_blocks(&self, key: &str) -> Result<Option<Vec<BlockType>>> {
        let mut conn = self.redis.get_multiplexed_async_connection().await?;
        let data: Option<Vec<u8>> = conn.get(key).await?;

        match data {
            Some(bytes) => {
                let blocks: Vec<BlockType> = serde_json::from_slice(&bytes)?;
                Ok(Some(blocks))
            }
            None => Ok(None),
        }
    }

    /// Cache blocks with TTL
    async fn cache_blocks(&self, key: &str, blocks: &[BlockType], ttl: u64) -> Result<()> {
        let mut conn = self.redis.get_multiplexed_async_connection().await?;
        let data = serde_json::to_vec(blocks)?;
        conn.set_ex::<_, _, ()>(key, data, ttl).await?;
        Ok(())
    }

    /// Get cached latest block number
    async fn get_cached_latest_block(&self, key: &str) -> Result<Option<u64>> {
        let mut conn = self.redis.get_multiplexed_async_connection().await?;
        let number: Option<u64> = conn.get(key).await?;
        Ok(number)
    }

    /// Cache latest block number
    async fn cache_latest_block(&self, key: &str, block_number: u64, ttl: u64) -> Result<()> {
        let mut conn = self.redis.get_multiplexed_async_connection().await?;
        conn.set_ex::<_, _, ()>(key, block_number, ttl).await?;
        Ok(())
    }
}

/// Cached blockchain client wrapper
#[derive(Clone)]
pub struct CachedBlockClient<C: BlockChainClient> {
    inner_client: Arc<C>,
    cache: Arc<BlockCacheService>,
    network_slug: String,
    _chain_type: BlockChainType,
}

impl<C: BlockChainClient> CachedBlockClient<C> {
    pub fn new(inner_client: C, cache: Arc<BlockCacheService>, network: &Network) -> Self {
        Self {
            inner_client: Arc::new(inner_client),
            cache,
            network_slug: network.slug.clone(),
            _chain_type: network.network_type.clone(),
        }
    }

    fn block_cache_key(&self, start: u64, end: Option<u64>) -> String {
        format!(
            "{}:blocks:{}:{}:{:?}",
            self.cache.config.key_prefix, self.network_slug, start, end
        )
    }

    fn latest_block_cache_key(&self) -> String {
        format!(
            "{}:latest:{}",
            self.cache.config.key_prefix, self.network_slug
        )
    }
}

#[async_trait]
impl<C: BlockChainClient + Send + Sync> BlockChainClient for CachedBlockClient<C> {
    #[instrument(skip(self), fields(network = %self.network_slug))]
    async fn get_blocks(
        &self,
        start: u64,
        end: Option<u64>,
    ) -> Result<Vec<BlockType>, anyhow::Error> {
        let cache_key = self.block_cache_key(start, end);

        // Check cache first
        match self.cache.get_cached_blocks(&cache_key).await {
            Ok(Some(blocks)) => {
                debug!("Cache hit for blocks {} to {:?}", start, end);
                return Ok(blocks);
            }
            Ok(None) => {
                debug!("Cache miss for blocks {} to {:?}", start, end);
            }
            Err(e) => {
                debug!("Cache error, fetching from RPC: {}", e);
            }
        }

        // Fetch from RPC
        let blocks = self.inner_client.get_blocks(start, end).await?;

        // Cache the result
        if let Err(e) = self
            .cache
            .cache_blocks(&cache_key, &blocks, self.cache.config.block_ttl)
            .await
        {
            debug!("Failed to cache blocks: {}", e);
        }

        Ok(blocks)
    }

    #[instrument(skip(self), fields(network = %self.network_slug))]
    async fn get_latest_block_number(&self) -> Result<u64, anyhow::Error> {
        let cache_key = self.latest_block_cache_key();

        // Check cache first
        match self.cache.get_cached_latest_block(&cache_key).await {
            Ok(Some(number)) => {
                debug!("Cache hit for latest block number: {}", number);
                return Ok(number);
            }
            Ok(None) => {
                debug!("Cache miss for latest block number");
            }
            Err(e) => {
                debug!("Cache error, fetching from RPC: {}", e);
            }
        }

        // Fetch from RPC
        let block_number = self.inner_client.get_latest_block_number().await?;

        // Cache the result
        if let Err(e) = self
            .cache
            .cache_latest_block(&cache_key, block_number, self.cache.config.latest_block_ttl)
            .await
        {
            debug!("Failed to cache latest block number: {}", e);
        }

        Ok(block_number)
    }

    async fn get_contract_spec(
        &self,
        contract_id: &str,
    ) -> Result<openzeppelin_monitor::models::ContractSpec, anyhow::Error> {
        // Contract specs are not cached as they don't change
        self.inner_client.get_contract_spec(contract_id).await
    }
}
