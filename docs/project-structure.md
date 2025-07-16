# Project Structure

The OZ Monitor Orchestrator follows a clean, modular architecture similar to OpenZeppelin Monitor:

```bash
oz-monitor-orchestrator/
├── src/
│   ├── config/           # Configuration structures
│   │   ├── api.rs        # API server configuration
│   │   ├── block_cache.rs # Block cache configuration
│   │   ├── block_watcher.rs # Block watcher configuration
│   │   ├── error.rs      # Configuration errors
│   │   ├── load_balancer.rs # Load balancer configuration
│   │   ├── orchestrator.rs # Main orchestrator config
│   │   ├── service_mode.rs # Service mode enum
│   │   └── worker.rs     # Worker configuration
│   │
│   ├── models/           # Data models
│   │   ├── assignment.rs # Tenant-worker assignments
│   │   ├── error.rs      # Model validation errors
│   │   ├── metrics.rs    # Performance metrics
│   │   └── tenant.rs     # Tenant information
│   │
│   ├── repositories/     # Data access layer
│   │   ├── error.rs      # Repository errors
│   │   └── tenant.rs     # Tenant-aware repositories
│   │
│   ├── services/         # Business logic
│   │   ├── block_cache.rs # Redis block caching
│   │   ├── error.rs      # Service errors
│   │   ├── load_balancer.rs # Tenant distribution
│   │   ├── shared_block_watcher.rs # Block fetching
│   │   └── worker_pool.rs # Worker management
│   │
│   ├── lib.rs            # Library root
│   └── main.rs           # Application entry point
│
├── k8s/                  # Kubernetes manifests
│   ├── namespace.yaml
│   ├── redis-statefulset.yaml
│   ├── worker-deployment.yaml
│   ├── shared-block-watcher.yaml
│   ├── hpa.yaml
│   └── monitoring.yaml
│
├── examples/             # Example configurations
├── tests/                # Integration tests
└── docs/                 # Documentation
```

## Module Organization

### Config Module

Contains all configuration structures with validation logic. Each file represents a specific configuration domain.

### Models Module

Data structures used throughout the application. Follows the principle of keeping related types together.

### Repositories Module

Implements OpenZeppelin Monitor's repository traits with multi-tenant support, providing database-backed storage instead of file-based.

### Services Module

Core business logic organized by functionality. Each service has a clear, single responsibility.

## Naming Conventions

- **Files**: Lowercase with underscores, no redundant suffixes
- **Structs**: PascalCase
- **Functions**: snake_case
- **Constants**: SCREAMING_SNAKE_CASE

## Error Handling

Each module has its own error type:

- `ConfigError` - Configuration validation errors
- `ModelError` - Model validation errors
- `RepositoryError` - Database and data access errors
- `ServiceError` - Business logic errors

This provides clear error boundaries and better error messages.
