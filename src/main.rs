//! OpenZeppelin Monitor Orchestrator
//!
//! This service orchestrates multiple OpenZeppelin Monitor instances
//! for multi-tenant blockchain monitoring with efficient resource sharing.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use openzeppelin_monitor::repositories::NetworkRepositoryTrait;
use std::sync::Arc;
use tokio::signal;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use oz_monitor_orchestrator::{
    config::{OrchestratorConfig, ServiceMode},
    repositories::TenantAwareNetworkRepository,
    services::{
        block_cache::BlockCacheService, cached_client_pool::CachedClientPool,
        load_balancer::LoadBalancer, oz_monitor_integration::OzMonitorServices,
        shared_block_watcher::SharedBlockWatcher, worker_pool::MonitorWorkerPool,
    },
};

#[derive(Parser)]
#[command(name = "oz-monitor-orchestrator")]
#[command(about = "Multi-tenant orchestrator for OpenZeppelin Monitor", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Run as a monitor worker
    Worker,
    /// Run as a shared block watcher
    BlockWatcher,
    /// Run the management API
    Api,
    /// Run all services (for development)
    All,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Parse CLI arguments
    let cli = Cli::parse();

    // Load configuration
    let config = OrchestratorConfig::load().context("Failed to load configuration")?;

    config.validate().map_err(|e| anyhow::anyhow!("Invalid configuration: {}", e))?;

    info!("Starting OZ Monitor Orchestrator");

    // Determine service mode
    let service_mode = match cli.command {
        Some(Commands::Worker) => ServiceMode::Worker,
        Some(Commands::BlockWatcher) => ServiceMode::BlockWatcher,
        Some(Commands::Api) => ServiceMode::Api,
        Some(Commands::All) => ServiceMode::All,
        None => config.service_mode.clone(),
    };

    // Connect to database
    let db_pool = Arc::new(
        sqlx::PgPool::connect(&config.database_url)
            .await
            .context("Failed to connect to database")?,
    );

    // Initialize services based on mode
    match service_mode {
        ServiceMode::Worker => run_worker(config, db_pool).await?,
        ServiceMode::BlockWatcher => run_block_watcher(config, db_pool).await?,
        ServiceMode::Api => run_api(config, db_pool).await?,
        ServiceMode::All => run_all(config, db_pool).await?,
    }

    Ok(())
}

async fn run_worker(config: OrchestratorConfig, db_pool: Arc<sqlx::PgPool>) -> Result<()> {
    info!("Starting in Worker mode");

    // Initialize block cache
    let cache = Arc::new(
        BlockCacheService::new(&config.redis_url, config.block_cache.into())
            .await
            .context("Failed to initialize block cache")?,
    );

    // Initialize cached client pool
    let client_pool = Arc::new(CachedClientPool::new(cache.clone()));

    // Initialize shared block watcher to receive block events
    let block_watcher = Arc::new(SharedBlockWatcher::new(
        cache.clone(),
        config.block_watcher.into(),
    ));

    // Initialize worker pool
    let worker_pool = MonitorWorkerPool::new(db_pool.clone(), cache.clone(), config.worker.into());

    // Initialize load balancer
    let load_balancer = Arc::new(LoadBalancer::new(config.load_balancer.into()));

    // Get worker ID from environment or generate
    let worker_id =
        std::env::var("WORKER_ID").unwrap_or_else(|_| format!("worker-{}", uuid::Uuid::new_v4()));

    info!("Worker ID: {}", worker_id);

    // Register with load balancer
    load_balancer.add_worker(worker_id.clone()).await?;

    // Get initial tenant assignments
    let assigned_tenants = load_balancer.get_worker_assignments(&worker_id).await?;
    info!("Worker {} assigned {} tenants", worker_id, assigned_tenants.len());

    // Create and start the worker
    worker_pool.create_worker(
        worker_id.clone(),
        assigned_tenants,
        block_watcher.clone(),
        client_pool,
    ).await?;

    info!("Worker started successfully");
    wait_for_shutdown().await;

    Ok(())
}

async fn run_block_watcher(config: OrchestratorConfig, db_pool: Arc<sqlx::PgPool>) -> Result<()> {
    info!("Starting in Block Watcher mode");

    // Initialize block cache
    let cache = Arc::new(
        BlockCacheService::new(&config.redis_url, config.block_cache.into())
            .await
            .context("Failed to initialize block cache")?,
    );

    // Initialize cached client pool
    let client_pool = Arc::new(CachedClientPool::new(cache.clone()));

    // Initialize shared block watcher
    let block_watcher = Arc::new(SharedBlockWatcher::new(cache.clone(), config.block_watcher.into()));

    // Initialize OZ Monitor services to get network configurations
    // In block watcher mode, we need all tenant IDs to get all networks
    let all_tenant_ids = get_all_tenant_ids(&db_pool).await?;
    let oz_services = Arc::new(
        OzMonitorServices::new(db_pool.clone(), all_tenant_ids.clone(), client_pool.clone())
            .await
            .context("Failed to initialize OZ Monitor services")?
    );

    // Get all active networks from OZ services
    let active_networks = oz_services.get_active_networks().await?;
    
    // Load network configurations from database
    let network_repo = TenantAwareNetworkRepository::new(db_pool.clone(), all_tenant_ids);
    let all_networks = network_repo.get_all();
    
    // Add networks with active monitors to the block watcher
    for slug in active_networks {
        if let Some(network) = all_networks.get(&slug) {
            block_watcher.add_network(network.clone()).await?;
            info!("Added network {} to block watcher", slug);
        }
    }

    // Start watching blocks
    block_watcher.start(client_pool).await?;

    info!("Block watcher started successfully");
    wait_for_shutdown().await;

    Ok(())
}

/// Get all tenant IDs from the database
async fn get_all_tenant_ids(db_pool: &sqlx::PgPool) -> Result<Vec<uuid::Uuid>> {
    let tenant_ids = sqlx::query_scalar::<_, uuid::Uuid>(
        "SELECT DISTINCT tenant_id FROM tenant_monitors WHERE is_active = true"
    )
    .fetch_all(db_pool)
    .await
    .context("Failed to fetch tenant IDs")?;
    
    Ok(tenant_ids)
}

async fn run_api(config: OrchestratorConfig, _db_pool: Arc<sqlx::PgPool>) -> Result<()> {
    info!("Starting in API mode");

    // TODO: Implement API server with endpoints for:
    // - Worker management
    // - Tenant assignment
    // - Metrics and monitoring
    // - Manual rebalancing

    let addr = format!("{}:{}", config.api.host, config.api.port);
    info!("API server listening on {}", addr);

    wait_for_shutdown().await;

    Ok(())
}

async fn run_all(config: OrchestratorConfig, db_pool: Arc<sqlx::PgPool>) -> Result<()> {
    info!("Starting all services");

    // In production, these would be separate processes
    // For development, we can run them all in one process

    let worker_handle = tokio::spawn({
        let config = config.clone();
        let db_pool = db_pool.clone();
        async move {
            if let Err(e) = run_worker(config, db_pool).await {
                error!("Worker failed: {}", e);
            }
        }
    });

    let block_watcher_handle = tokio::spawn({
        let config = config.clone();
        let db_pool = db_pool.clone();
        async move {
            if let Err(e) = run_block_watcher(config, db_pool).await {
                error!("Block watcher failed: {}", e);
            }
        }
    });

    let api_handle = tokio::spawn({
        let config = config.clone();
        let db_pool = db_pool.clone();
        async move {
            if let Err(e) = run_api(config, db_pool).await {
                error!("API server failed: {}", e);
            }
        }
    });

    // Wait for any service to fail
    tokio::select! {
        _ = worker_handle => error!("Worker exited"),
        _ = block_watcher_handle => error!("Block watcher exited"),
        _ = api_handle => error!("API server exited"),
    }

    Ok(())
}

async fn wait_for_shutdown() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("Received Ctrl+C, shutting down");
        }
        _ = terminate => {
            info!("Received SIGTERM, shutting down");
        }
    }
}
