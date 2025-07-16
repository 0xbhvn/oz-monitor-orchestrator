//! Shared Block Watcher Service
//!
//! A single block watcher per network that fetches blocks once and
//! distributes them to all worker instances.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tracing::{error, info, instrument, warn};

// Import OpenZeppelin Monitor types
use openzeppelin_monitor::{
    models::{BlockType, Network},
    services::blockchain::{BlockChainClient, ClientPoolTrait},
};

use crate::services::block_cache::BlockCacheService;

/// Block event sent to workers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockEvent {
    pub network: Network,
    pub blocks: Vec<BlockType>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Shared block watcher configuration
#[derive(Debug, Clone)]
pub struct SharedBlockWatcherConfig {
    /// Channel buffer size
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

/// Network watcher state
struct NetworkWatcherState {
    network: Network,
    last_processed_block: u64,
    is_running: bool,
}

/// Shared block watcher that fetches blocks once per network
pub struct SharedBlockWatcher {
    networks: Arc<RwLock<HashMap<String, NetworkWatcherState>>>,
    block_sender: broadcast::Sender<BlockEvent>,
    cache: Arc<BlockCacheService>,
    config: SharedBlockWatcherConfig,
}

impl SharedBlockWatcher {
    pub fn new(cache: Arc<BlockCacheService>, config: SharedBlockWatcherConfig) -> Self {
        let (block_sender, _) = broadcast::channel(config.channel_buffer_size);

        Self {
            networks: Arc::new(RwLock::new(HashMap::new())),
            block_sender,
            cache,
            config,
        }
    }

    /// Subscribe to block events
    pub fn subscribe(&self) -> broadcast::Receiver<BlockEvent> {
        self.block_sender.subscribe()
    }

    /// Add a network to watch
    pub async fn add_network(&self, network: Network) -> Result<()> {
        let mut networks = self.networks.write().await;

        if networks.contains_key(&network.slug) {
            info!("Network {} already being watched", network.slug);
            return Ok(());
        }

        let state = NetworkWatcherState {
            network: network.clone(),
            last_processed_block: 0,
            is_running: false,
        };

        networks.insert(network.slug.clone(), state);
        info!("Added network {} to shared block watcher", network.slug);

        Ok(())
    }

    /// Remove a network from watching
    pub async fn remove_network(&self, network_slug: &str) -> Result<()> {
        let mut networks = self.networks.write().await;

        if let Some(mut state) = networks.remove(network_slug) {
            state.is_running = false;
            info!("Removed network {} from shared block watcher", network_slug);
        }

        Ok(())
    }

    /// Start watching all networks
    #[instrument(skip(self, client_pool))]
    pub async fn start<CP: ClientPoolTrait + Send + Sync + 'static>(
        &self,
        client_pool: Arc<CP>,
    ) -> Result<()> {
        info!("Starting shared block watcher");

        let networks = self.networks.read().await;
        let mut handles = Vec::new();

        // Start a watcher task for each network
        for (_network_slug, state) in networks.iter() {
            if state.is_running {
                continue;
            }

            let handle = self
                .start_network_watcher(state.network.clone(), client_pool.clone())
                .await?;

            handles.push(handle);
        }

        // Wait for all watchers (they should run forever)
        futures::future::join_all(handles).await;

        Ok(())
    }

    /// Start watcher for a specific network
    async fn start_network_watcher<CP: ClientPoolTrait + Send + Sync + 'static>(
        &self,
        network: Network,
        client_pool: Arc<CP>,
    ) -> Result<tokio::task::JoinHandle<()>> {
        let networks = self.networks.clone();
        let block_sender = self.block_sender.clone();
        let cache = self.cache.clone();
        let config = self.config.clone();
        let network_slug = network.slug.clone();

        // Mark as running
        {
            let mut networks_lock = networks.write().await;
            if let Some(state) = networks_lock.get_mut(&network_slug) {
                state.is_running = true;
            }
        }

        let handle = tokio::spawn(async move {
            info!("Starting watcher for network {}", network_slug);

            loop {
                // Check if we should continue
                {
                    let networks_lock = networks.read().await;
                    if let Some(state) = networks_lock.get(&network_slug) {
                        if !state.is_running {
                            info!("Stopping watcher for network {}", network_slug);
                            break;
                        }
                    } else {
                        warn!("Network {} removed, stopping watcher", network_slug);
                        break;
                    }
                }

                // Fetch and process blocks
                match fetch_and_broadcast_blocks(
                    &network,
                    &networks,
                    &client_pool,
                    &block_sender,
                    &cache,
                    &config,
                )
                .await
                {
                    Ok(blocks_processed) => {
                        if blocks_processed > 0 {
                            info!(
                                "Processed {} blocks for network {}",
                                blocks_processed, network_slug
                            );
                        }
                    }
                    Err(e) => {
                        error!(
                            "Error processing blocks for network {}: {}",
                            network_slug, e
                        );
                    }
                }

                // Sleep based on network's cron schedule or default interval
                let sleep_duration = calculate_sleep_duration(&network);
                tokio::time::sleep(sleep_duration).await;
            }

            // Mark as not running
            let mut networks_lock = networks.write().await;
            if let Some(state) = networks_lock.get_mut(&network_slug) {
                state.is_running = false;
            }
        });

        Ok(handle)
    }
}

/// Fetch blocks and broadcast to subscribers
async fn fetch_and_broadcast_blocks<CP: ClientPoolTrait>(
    network: &Network,
    networks: &Arc<RwLock<HashMap<String, NetworkWatcherState>>>,
    client_pool: &Arc<CP>,
    block_sender: &broadcast::Sender<BlockEvent>,
    _cache: &Arc<BlockCacheService>,
    config: &SharedBlockWatcherConfig,
) -> Result<usize> {
    // Get the last processed block
    let last_processed_block = {
        let networks_lock = networks.read().await;
        networks_lock
            .get(&network.slug)
            .map(|s| s.last_processed_block)
            .unwrap_or(0)
    };

    // Process based on network type
    match network.network_type {
        openzeppelin_monitor::models::BlockChainType::EVM => {
            let client = client_pool
                .get_evm_client(network)
                .await
                .context("Failed to get EVM client")?;
            
            fetch_blocks_for_client(
                client.as_ref(),
                network,
                last_processed_block,
                config,
                block_sender,
                networks,
            )
            .await
        }
        openzeppelin_monitor::models::BlockChainType::Stellar => {
            let client = client_pool
                .get_stellar_client(network)
                .await
                .context("Failed to get Stellar client")?;
            
            fetch_blocks_for_client(
                client.as_ref(),
                network,
                last_processed_block,
                config,
                block_sender,
                networks,
            )
            .await
        }
        _ => {
            warn!("Unsupported network type for {}", network.slug);
            Ok(0)
        }
    }
}

/// Fetch blocks for a specific client type
async fn fetch_blocks_for_client<C: BlockChainClient>(
    client: &C,
    network: &Network,
    last_processed_block: u64,
    config: &SharedBlockWatcherConfig,
    block_sender: &broadcast::Sender<BlockEvent>,
    networks: &Arc<RwLock<HashMap<String, NetworkWatcherState>>>,
) -> Result<usize> {
    // Get latest block number
    let latest_block = retry_with_backoff(
        || client.get_latest_block_number(),
        config.retry_attempts,
        config.retry_delay_ms,
    )
    .await?;

    let latest_confirmed_block = latest_block.saturating_sub(network.confirmation_blocks);

    // Calculate block range to fetch
    let start_block = if last_processed_block == 0 {
        // First run - get only the latest confirmed block
        latest_confirmed_block
    } else {
        last_processed_block + 1
    };

    if start_block > latest_confirmed_block {
        // No new blocks to process
        return Ok(0);
    }

    // Limit the number of blocks to fetch
    let end_block = std::cmp::min(
        latest_confirmed_block,
        start_block + config.max_blocks_per_fetch - 1,
    );

    // Fetch blocks
    let blocks = retry_with_backoff(
        || client.get_blocks(start_block, Some(end_block)),
        config.retry_attempts,
        config.retry_delay_ms,
    )
    .await?;

    if blocks.is_empty() {
        return Ok(0);
    }

    // Create block event
    let event = BlockEvent {
        network: network.clone(),
        blocks: blocks.clone(),
        timestamp: chrono::Utc::now(),
    };

    // Broadcast to all subscribers
    match block_sender.send(event) {
        Ok(receiver_count) => {
            info!(
                "Broadcast {} blocks for network {} to {} subscribers",
                blocks.len(),
                network.slug,
                receiver_count
            );
        }
        Err(_) => {
            warn!(
                "No subscribers for block events on network {}",
                network.slug
            );
        }
    }

    // Update last processed block
    {
        let mut networks_lock = networks.write().await;
        if let Some(state) = networks_lock.get_mut(&network.slug) {
            state.last_processed_block = end_block;
        }
    }

    Ok(blocks.len())
}

/// Calculate sleep duration based on network configuration
fn calculate_sleep_duration(network: &Network) -> std::time::Duration {
    // Parse cron schedule to determine interval
    // For now, use a simple default based on network type
    match network.network_type {
        openzeppelin_monitor::models::BlockChainType::EVM => {
            // Most EVM chains have ~12-15 second block times
            std::time::Duration::from_secs(15)
        }
        openzeppelin_monitor::models::BlockChainType::Stellar => {
            // Stellar has ~5 second block times
            std::time::Duration::from_secs(5)
        }
        _ => std::time::Duration::from_secs(30),
    }
}

/// Retry a future with exponential backoff
async fn retry_with_backoff<F, Fut, T, E>(
    mut f: F,
    max_attempts: u32,
    base_delay_ms: u64,
) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    let mut attempt = 0;
    loop {
        match f().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                attempt += 1;
                if attempt >= max_attempts {
                    return Err(anyhow::anyhow!(
                        "Failed after {} attempts: {}",
                        max_attempts,
                        e
                    ));
                }

                let delay = base_delay_ms * 2u64.pow(attempt - 1);
                warn!("Attempt {} failed: {}, retrying in {}ms", attempt, e, delay);
                tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
            }
        }
    }
}

