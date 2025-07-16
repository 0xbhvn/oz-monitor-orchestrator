# Claude Development Guide for OZ Monitor Orchestrator

## Project Overview

The OZ Monitor Orchestrator is a multi-tenant orchestration layer for OpenZeppelin Monitor that enables efficient resource sharing and horizontal scaling. It uses OpenZeppelin Monitor as a library without modifying its core codebase.

## Development Principles

### 1. Agile Development Mode

- Work in small, incremental steps
- Each component should be functional and testable
- Commit to making steady progress rather than perfect solutions

### 2. Continuous Validation

- Run `cargo check` at regular intervals to ensure code compiles
- Run `cargo build --release` after significant changes
- Test each component as you build it

### 3. Error-First Development

- Handle errors gracefully at every level
- Use the existing error types in each module:
  - `ConfigError` for configuration issues
  - `ModelError` for data validation
  - `RepositoryError` for database operations
  - `ServiceError` for business logic

## Code Style Guidelines

### 1. Follow Existing Patterns

- Study neighboring files before implementing new features
- Match the project's naming conventions:
  - Files: `lowercase_with_underscores.rs`
  - Structs: `PascalCase`
  - Functions: `snake_case`
  - Constants: `SCREAMING_SNAKE_CASE`

### 2. Module Organization

```bash
src/
├── config/      # Configuration structures with validation
├── models/      # Data structures and types
├── repositories/# Data access layer (implements OpenZeppelin Monitor traits)
├── services/    # Business logic (one responsibility per service)
├── lib.rs       # Library exports
└── main.rs      # Application entry point
```

### 3. Clean Architecture

- Keep layers separate and dependencies unidirectional
- Services depend on repositories and models
- Repositories depend only on models
- Models have no dependencies

## Development Workflow

### 1. Before Starting

```bash
# Ensure you're in the project directory
cd /Users/bhaven/Stellar/blip0/oz-monitor-orchestrator

# Check current state
cargo check
```

### 2. During Development

```bash
# After each significant change
cargo check

# When implementing a new feature
cargo test

# Before considering a feature complete
cargo build --release
```

### 3. Testing Strategy

- Write unit tests for individual components
- Use integration tests for service interactions
- Mock external dependencies (Redis, PostgreSQL)

## Key Components to Understand

### 1. Block Cache Service

- Redis-based caching for blockchain data
- Prevents duplicate RPC calls across workers
- Check `src/services/block_cache.rs`

### 2. Tenant-Aware Repositories

- Implements OpenZeppelin Monitor's repository interfaces
- Multi-tenant isolation at data layer
- Check `src/repositories/tenant.rs`

### 3. Worker Pool

- Manages multiple OpenZeppelin Monitor instances
- Health checking and tenant distribution
- Check `src/services/worker_pool.rs`

### 4. Load Balancer

- Distributes tenants across workers
- Multiple strategies (consistent hashing default)
- Check `src/services/load_balancer.rs`

## Common Tasks

### Adding a New Service

1. Create service file in `src/services/`
2. Define error type in `src/services/error.rs`
3. Implement service with single responsibility
4. Add unit tests
5. Export from `src/services/mod.rs`

### Adding Configuration

1. Add structure in appropriate `src/config/*.rs` file
2. Implement validation in the struct
3. Add to main config in `src/config/orchestrator.rs`
4. Update `config.yaml` example

### Implementing a Repository

1. Study OpenZeppelin Monitor's trait definition
2. Create implementation in `src/repositories/`
3. Add multi-tenant support (tenant_id filtering)
4. Handle PostgreSQL instead of file storage

## Performance Considerations

### 1. Caching Strategy

- Cache blocks in Redis with appropriate TTL
- Use consistent key naming: `oz_cache:network:block:number`
- Monitor cache hit rates

### 2. Worker Distribution

- Default to consistent hashing for tenant stability
- Monitor worker load distribution
- Set appropriate `max_tenants_per_worker`

### 3. Database Queries

- Use prepared statements
- Index on (tenant_id, is_active) for efficient filtering
- Batch operations where possible

## Debugging Tips

### 1. Enable Debug Logging

```bash
RUST_LOG=debug cargo run
```

### 2. Check Redis Connection

```bash
redis-cli ping
```

### 3. Verify Database Schema

```sql
-- Check tenant isolation tables exist
SELECT table_name FROM information_schema.tables 
WHERE table_schema = 'public';
```

## Security Considerations

### 1. Tenant Isolation

- Always filter by tenant_id in queries
- Validate tenant ownership before operations
- Use row-level security where applicable

### 2. Resource Limits

- Enforce per-tenant RPC limits
- Monitor resource usage per tenant
- Implement circuit breakers for failing tenants

### 3. Secrets Management

- Use Kubernetes secrets for credentials
- Never log sensitive information
- Rotate credentials regularly

## Integration Points

### 1. OpenZeppelin Monitor

- Used as a library dependency
- Implement its trait interfaces
- Respect its architectural patterns

### 2. Stellar Monitor Tenant Isolation

- Read tenant configurations from PostgreSQL
- Respect tenant limits and quotas
- Sync tenant state periodically

### 3. Kubernetes

- Design for horizontal scaling
- Use health checks and readiness probes
- Follow cloud-native patterns

## Quick Commands Reference

```bash
# Development
cargo check                    # Quick syntax check
cargo build                    # Debug build
cargo build --release          # Production build
cargo test                     # Run all tests
cargo test -- --nocapture      # See test output

# Running locally
cargo run -- worker            # Run as worker
cargo run -- block-watcher     # Run as block watcher
cargo run -- all               # Run all services

# Docker
docker build -t oz-monitor-orchestrator:latest .
docker-compose up -d           # Start dependencies

# Kubernetes
kubectl apply -f k8s/          # Deploy all resources
kubectl get pods -n oz-monitor-orchestrator
kubectl logs -f deployment/worker -n oz-monitor-orchestrator
```

## Remember

1. **Always validate** - Run `cargo check` frequently
2. **Test incrementally** - Each component should work independently
3. **Follow patterns** - Study existing code before implementing
4. **Handle errors** - Use appropriate error types for each layer
5. **Document complex logic** - But keep it concise
6. **Performance matters** - This is a high-throughput system
7. **Security first** - Tenant isolation is critical

When in doubt, refer to the existing codebase patterns and the OpenZeppelin Monitor documentation.
