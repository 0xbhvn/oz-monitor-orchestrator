//! Cached Client Pool Implementation
//!
//! Provides a ClientPoolTrait implementation that wraps blockchain clients
//! with caching capabilities for the orchestrator.
//! For now, this is a simple pass-through to the OpenZeppelin Monitor ClientPool
//! In the future, we can add caching capabilities here.

use anyhow::Result;
use async_trait::async_trait;
// use std::collections::HashMap;
use std::sync::Arc;
// use tokio::sync::RwLock;

use openzeppelin_monitor::{
    models::Network,
    // services::{
    //     blockchain::{
    //         BlockChainClient, BlockFilterFactory, ClientPoolTrait,
    //         EvmClient, EvmClientTrait, EVMTransportClient,
    //         StellarClient, StellarClientTrait, StellarTransportClient,
    //     },
    //     filter::BlockFilter,
    // },
    services::blockchain::{ClientPool, ClientPoolTrait},
};

use super::block_cache::BlockCacheService;

// /// Cached EVM client that wraps the base client with caching
// pub struct CachedEvmClient {
//     inner: Arc<EvmClient<EVMTransportClient>>,
//     cache: Arc<BlockCacheService>,
// }
// /// Cached Stellar client that wraps the base client with caching
// pub struct CachedStellarClient {
//     inner: Arc<StellarClient<StellarTransportClient>>,
//     cache: Arc<BlockCacheService>,
// }

/// Cached client pool implementation
pub struct CachedClientPool {
    // evm_clients: Arc<RwLock<HashMap<String, Arc<CachedEvmClient>>>>,
    // stellar_clients: Arc<RwLock<HashMap<String, Arc<CachedStellarClient>>>>,
    // cache: Arc<BlockCacheService>,
    inner: ClientPool,
    _cache: Arc<BlockCacheService>,
}

impl CachedClientPool {
    pub fn new(cache: Arc<BlockCacheService>) -> Self {
        Self {
            // evm_clients: Arc::new(RwLock::new(HashMap::new())),
            // stellar_clients: Arc::new(RwLock::new(HashMap::new())),
            // cache,
            inner: ClientPool::new(),
            _cache: cache,
        }
    }
}

#[async_trait]
impl ClientPoolTrait for CachedClientPool {
    type EvmClient = <ClientPool as ClientPoolTrait>::EvmClient;
    type StellarClient = <ClientPool as ClientPoolTrait>::StellarClient;

    async fn get_evm_client(&self, network: &Network) -> Result<Arc<Self::EvmClient>> {
        // let clients = self.evm_clients.read().await;
        // if let Some(client) = clients.get(&network.slug) {
        //     return Ok(client.clone());
        // }
        // drop(clients);

        // // Create new client
        // let mut clients = self.evm_clients.write().await;
        // // Check again in case another task created it
        // if let Some(client) = clients.get(&network.slug) {
        //     return Ok(client.clone());
        // }

        // // Create the base EVM client
        // let transport = EVMTransportClient::new(network.chain_id, network.rpc_urls.clone())?;
        // let base_client = Arc::new(EvmClient::new(transport));

        // let cached_client = Arc::new(CachedEvmClient {
        //     inner: base_client,
        //     cache: self.cache.clone(),
        // });

        // clients.insert(network.slug.clone(), cached_client.clone());
        // Ok(cached_client)

        self.inner.get_evm_client(network).await
    }

    async fn get_stellar_client(&self, network: &Network) -> Result<Arc<Self::StellarClient>> {
        // let clients = self.stellar_clients.read().await;
        // if let Some(client) = clients.get(&network.slug) {
        //     return Ok(client.clone());
        // }
        // drop(clients);

        // // Create new client
        // let mut clients = self.stellar_clients.write().await;
        // // Check again in case another task created it
        // if let Some(client) = clients.get(&network.slug) {
        //     return Ok(client.clone());
        // }

        // // Create the base Stellar client
        // let transport = StellarTransportClient::new(
        //     network.name.clone(),
        //     network.rpc_urls.clone(),
        //     network.soroban_rpc_urls.clone(),
        //     network.horizon_urls.clone(),
        // )?;
        // let base_client = Arc::new(StellarClient::new(transport));

        // let cached_client = Arc::new(CachedStellarClient {
        //     inner: base_client,
        //     cache: self.cache.clone(),
        // });

        // clients.insert(network.slug.clone(), cached_client.clone());
        // Ok(cached_client)

        self.inner.get_stellar_client(network).await
    }
}

// // Implement required traits for CachedEvmClient
// #[async_trait]
// impl EvmClientTrait for CachedEvmClient {
//     type Transport = EVMTransportClient;

//     fn transport(&self) -> &Self::Transport {
//         self.inner.transport()
//     }

//     async fn get_block_number(&self) -> Result<u64> {
//         self.inner.get_block_number().await
//     }

//     async fn get_block(&self, block_number: u64) -> Result<openzeppelin_monitor::models::EVMBlock> {
//         // Check cache first
//         if let Ok(Some(cached_block)) = self.cache.get_evm_block("", block_number).await {
//             return Ok(cached_block);
//         }

//         // Fetch from network
//         let block = self.inner.get_block(block_number).await?;

//         // Cache the result
//         if let Err(e) = self.cache.set_evm_block("", &block).await {
//             tracing::warn!("Failed to cache EVM block: {}", e);
//         }

//         Ok(block)
//     }

//     async fn get_transaction_receipt(
//         &self,
//         tx_hash: &str,
//     ) -> Result<openzeppelin_monitor::models::EVMTransactionReceipt> {
//         self.inner.get_transaction_receipt(tx_hash).await
//     }

//     async fn get_logs(
//         &self,
//         from_block: u64,
//         to_block: u64,
//         addresses: Option<Vec<String>>,
//         topics: Option<Vec<Option<Vec<String>>>>,
//     ) -> Result<Vec<openzeppelin_monitor::models::EVMReceiptLog>> {
//         self.inner
//             .get_logs(from_block, to_block, addresses, topics)
//             .await
//     }
// }

// #[async_trait]
// impl BlockChainClient for CachedEvmClient {
//     fn network_name(&self) -> &str {
//         self.inner.network_name()
//     }

//     fn as_any(&self) -> &dyn std::any::Any {
//         self
//     }
// }

// impl BlockFilterFactory<CachedEvmClient> for CachedEvmClient {
//     type Filter = <EvmClient<EVMTransportClient> as BlockFilterFactory<
//         EvmClient<EVMTransportClient>,
//     >>::Filter;

//     fn filter() -> Self::Filter {
//         EvmClient::<EVMTransportClient>::filter()
//     }
// }

// // Implement required traits for CachedStellarClient
// #[async_trait]
// impl StellarClientTrait for CachedStellarClient {
//     type Transport = StellarTransportClient;

//     fn transport(&self) -> &Self::Transport {
//         self.inner.transport()
//     }

//     async fn get_latest_ledger(&self) -> Result<u32> {
//         self.inner.get_latest_ledger().await
//     }

//     async fn get_ledger(
//         &self,
//         sequence: u32,
//     ) -> Result<openzeppelin_monitor::models::StellarBlock> {
//         // Check cache first
//         if let Ok(Some(cached_block)) = self.cache.get_stellar_block("", sequence).await {
//             return Ok(cached_block);
//         }

//         // Fetch from network
//         let block = self.inner.get_ledger(sequence).await?;

//         // Cache the result
//         if let Err(e) = self.cache.set_stellar_block("", &block).await {
//             tracing::warn!("Failed to cache Stellar block: {}", e);
//         }

//         Ok(block)
//     }

//     async fn get_transaction(
//         &self,
//         tx_hash: &str,
//     ) -> Result<openzeppelin_monitor::models::StellarTransaction> {
//         self.inner.get_transaction(tx_hash).await
//     }

//     async fn get_events(
//         &self,
//         start_ledger: u32,
//         filters: Option<Vec<String>>,
//     ) -> Result<Vec<openzeppelin_monitor::models::StellarEvent>> {
//         self.inner.get_events(start_ledger, filters).await
//     }

//     async fn simulate_transaction(&self, tx_xdr: &str) -> Result<serde_json::Value> {
//         self.inner.simulate_transaction(tx_xdr).await
//     }
// }

// #[async_trait]
// impl BlockChainClient for CachedStellarClient {
//     fn network_name(&self) -> &str {
//         self.inner.network_name()
//     }

//     fn as_any(&self) -> &dyn std::any::Any {
//         self
//     }
// }

// impl BlockFilterFactory<CachedStellarClient> for CachedStellarClient {
//     type Filter = <StellarClient<StellarTransportClient> as BlockFilterFactory<
//         StellarClient<StellarTransportClient>,
//     >>::Filter;

//     fn filter() -> Self::Filter {
//         StellarClient::<StellarTransportClient>::filter()
//     }
// }
