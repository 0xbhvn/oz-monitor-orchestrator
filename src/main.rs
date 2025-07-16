//! OpenZeppelin Monitor Orchestrator
//!
//! This service orchestrates multiple OpenZeppelin Monitor instances
//! for multi-tenant blockchain monitoring with efficient resource sharing.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::sync::Arc;
use tokio::signal;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use oz_monitor_orchestrator::{
    config::{OrchestratorConfig, ServiceMode},
    services::{
        block_cache::BlockCacheService, load_balancer::LoadBalancer,
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

    // Initialize worker pool
    let _worker_pool = MonitorWorkerPool::new(db_pool.clone(), cache.clone(), config.worker.into());

    // Initialize load balancer
    let load_balancer = Arc::new(LoadBalancer::new(config.load_balancer.into()));

    // Get worker ID from environment or generate
    let worker_id =
        std::env::var("WORKER_ID").unwrap_or_else(|_| format!("worker-{}", uuid::Uuid::new_v4()));

    info!("Worker ID: {}", worker_id);

    // Register with load balancer
    load_balancer.add_worker(worker_id.clone()).await?;

    // TODO: In a real implementation, you would:
    // 1. Connect to the shared block watcher
    // 2. Get tenant assignments from load balancer
    // 3. Initialize OpenZeppelin monitor client pool
    // 4. Start processing assigned tenants

    // For now, just wait for shutdown
    info!("Worker started successfully");
    wait_for_shutdown().await;

    Ok(())
}

async fn run_block_watcher(config: OrchestratorConfig, _db_pool: Arc<sqlx::PgPool>) -> Result<()> {
    info!("Starting in Block Watcher mode");

    // Initialize block cache
    let cache = Arc::new(
        BlockCacheService::new(&config.redis_url, config.block_cache.into())
            .await
            .context("Failed to initialize block cache")?,
    );

    // Initialize shared block watcher
    let _block_watcher = SharedBlockWatcher::new(cache.clone(), config.block_watcher.into());

    // TODO: In a real implementation, you would:
    // 1. Load network configurations from database
    // 2. Initialize OpenZeppelin monitor client pool
    // 3. Add networks to the block watcher
    // 4. Start watching blocks

    info!("Block watcher started successfully");
    wait_for_shutdown().await;

    Ok(())
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
