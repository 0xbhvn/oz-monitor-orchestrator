# Project Structure

The OZ Monitor Orchestrator follows a clean, modular architecture similar to OpenZeppelin Monitor:

```bash
oz-monitor-orchestrator/
├── src/
│   ├── config/                     # Configuration structures
│   │   ├── mod.rs                  # Module exports
│   │   ├── api.rs                  # API server configuration
│   │   ├── block_cache.rs          # Block cache configuration
│   │   ├── block_watcher.rs        # Block watcher configuration
│   │   ├── error.rs                # Configuration errors
│   │   ├── load_balancer.rs        # Load balancer configuration
│   │   ├── orchestrator.rs         # Main orchestrator config
│   │   ├── service_mode.rs         # Service mode enum
│   │   └── worker.rs               # Worker configuration
│   │
│   ├── models/                     # Data models
│   │   ├── mod.rs                  # Module exports
│   │   ├── assignment.rs           # Tenant-worker assignments
│   │   ├── error.rs                # Model validation errors
│   │   ├── metrics.rs              # Performance metrics
│   │   └── tenant.rs               # Tenant information
│   │
│   ├── repositories/               # Data access layer
│   │   ├── mod.rs                  # Module exports
│   │   ├── error.rs                # Repository errors
│   │   └── tenant.rs               # Tenant-aware repositories
│   │
│   ├── services/                   # Business logic
│   │   ├── mod.rs                  # Module exports
│   │   ├── block_cache.rs          # Redis block caching
│   │   ├── cached_client_pool.rs   # Client pool with caching
│   │   ├── error.rs                # Service errors
│   │   ├── load_balancer.rs        # Tenant distribution
│   │   ├── oz_monitor_integration.rs # OpenZeppelin Monitor integration
│   │   ├── shared_block_watcher.rs # Block fetching coordination
│   │   └── worker_pool.rs          # Worker management
│   │
│   ├── lib.rs                      # Library root
│   └── main.rs                     # Application entry point
│
├── k8s/                            # Kubernetes manifests
│   ├── namespace.yaml              # Namespace definition
│   ├── secrets.yaml                # Secret configurations
│   ├── redis-statefulset.yaml      # Redis deployment
│   ├── worker-deployment.yaml      # Worker deployment
│   ├── shared-block-watcher.yaml   # Block watcher deployment
│   ├── hpa.yaml                    # Horizontal pod autoscaler
│   └── monitoring.yaml             # Prometheus monitoring
│
├── scripts/                        # Utility scripts
│   ├── start.sh                    # Start the demo system
│   ├── status.sh                   # Check system status
│   ├── test-api.sh                 # Test API endpoints
│   └── verify.sh                   # Verify system components
│
├── sql/                            # SQL scripts
│   └── assign_tenant_to_worker.sql # Tenant assignment queries
│
├── demo/                           # Demo artifacts
│   ├── orchestrator.log            # Orchestrator logs
│   └── tenant-isolation.log        # Tenant API logs
│
├── docs/                           # Documentation
│   ├── architecture.md             # System architecture
│   ├── multi-tenant.md             # Multi-tenant design
│   └── structure.md                # This file
│
├── config.yaml                     # Example configuration
├── docker-compose.yml              # Docker development setup
├── Dockerfile                      # Container image
├── Cargo.toml                      # Rust project manifest
└── Cargo.lock                      # Dependency lock file
```

## Module Organization

### Config Module (`src/config/`)

Contains all configuration structures with validation logic. Each file represents a specific configuration domain:

- **api.rs**: API server settings (host, port)
- **block_cache.rs**: Redis cache TTL and key prefixes
- **block_watcher.rs**: Block fetching parameters
- **load_balancer.rs**: Tenant distribution strategies
- **orchestrator.rs**: Main configuration aggregator
- **service_mode.rs**: Execution mode selection
- **worker.rs**: Worker pool settings

### Models Module (`src/models/`)

Data structures used throughout the application:

- **assignment.rs**: Tenant-to-worker mapping structures
- **metrics.rs**: Performance and monitoring metrics
- **tenant.rs**: Tenant identification and metadata

### Repositories Module (`src/repositories/`)

Implements OpenZeppelin Monitor's repository traits with multi-tenant support:

- **tenant.rs**: Multi-tenant aware implementations of NetworkRepository, MonitorRepository, and TriggerRepository

### Services Module (`src/services/`)

Core business logic organized by functionality:

- **block_cache.rs**: Redis-based block caching to prevent duplicate RPC calls
- **cached_client_pool.rs**: Client pool implementation with caching support
- **load_balancer.rs**: Tenant distribution across workers (consistent hashing, round-robin)
- **oz_monitor_integration.rs**: Core integration with OpenZeppelin Monitor library
- **shared_block_watcher.rs**: Coordinates block fetching across networks
- **worker_pool.rs**: Manages monitor worker instances

## Key Files

### main.rs

Application entry point supporting multiple service modes:

- `worker` - Run as a monitor worker
- `block-watcher` - Run as shared block watcher
- `api` - Run management API (not yet implemented)
- `all` - Run all services (development mode)

### lib.rs

Library exports for using the orchestrator as a dependency.

## Naming Conventions

- **Files**: Lowercase with underscores (`snake_case`)
- **Modules**: No redundant suffixes (e.g., `error.rs` not `errors.rs`)
- **Structs**: PascalCase
- **Functions**: snake_case
- **Constants**: SCREAMING_SNAKE_CASE

## Error Handling

Each module has its own error type for clear error boundaries:

- `ConfigError` - Configuration validation errors
- `ModelError` - Model validation errors  
- `RepositoryError` - Database and data access errors
- `ServiceError` - Business logic errors

All errors implement proper error chaining using `thiserror`.

## Integration Points

The orchestrator integrates with:

1. **OpenZeppelin Monitor** - Used as a library dependency
2. **PostgreSQL** - Tenant configuration storage
3. **Redis** - Block caching and coordination
4. **Kubernetes** - Deployment and scaling platform
