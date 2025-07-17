//! Cached Client Pool Implementation
//!
//! Provides a ClientPoolTrait implementation that includes caching capabilities
//! for the orchestrator. This implementation uses a pass-through approach for
//! client creation while exposing access to the block cache service.
//!
//! The caching strategy is implemented at a higher level (in SharedBlockWatcher)
//! rather than wrapping individual clients, which simplifies the implementation
//! while still providing the performance benefits of caching.

use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;

use openzeppelin_monitor::{
    models::Network,
    services::blockchain::{ClientPool, ClientPoolTrait},
};

use super::block_cache::BlockCacheService;

/// Cached client pool implementation
///
/// This implementation provides a caching layer over the standard ClientPool.
/// While we pass through client creation to the underlying pool, we could
/// enhance this in the future to provide caching at the client level.
pub struct CachedClientPool {
    /// The underlying client pool
    inner: ClientPool,
    /// Block cache service for caching blockchain data
    cache: Arc<BlockCacheService>,
}

impl CachedClientPool {
    /// Create a new cached client pool
    pub fn new(cache: Arc<BlockCacheService>) -> Self {
        Self {
            inner: ClientPool::new(),
            cache,
        }
    }

    /// Get the cache service
    pub fn cache(&self) -> Arc<BlockCacheService> {
        self.cache.clone()
    }
}

#[async_trait]
impl ClientPoolTrait for CachedClientPool {
    type EvmClient = <ClientPool as ClientPoolTrait>::EvmClient;
    type StellarClient = <ClientPool as ClientPoolTrait>::StellarClient;

    async fn get_evm_client(&self, network: &Network) -> Result<Arc<Self::EvmClient>> {
        // Pass through to the underlying pool
        // Caching is handled at the SharedBlockWatcher level
        self.inner.get_evm_client(network).await
    }

    async fn get_stellar_client(&self, network: &Network) -> Result<Arc<Self::StellarClient>> {
        // Pass through to the underlying pool
        // Caching is handled at the SharedBlockWatcher level
        self.inner.get_stellar_client(network).await
    }
}
