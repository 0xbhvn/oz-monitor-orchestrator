# OpenZeppelin Monitor Orchestrator

A multi-tenant orchestration layer for OpenZeppelin Monitor that enables efficient resource sharing and horizontal scaling across multiple tenants without modifying the core OpenZeppelin Monitor codebase.

## Architecture Overview

The orchestrator uses OpenZeppelin Monitor as a library and provides:

1. **Shared Block Fetching**: One block watcher per network prevents duplicate RPC calls
2. **Distributed Processing**: Worker pool architecture for parallel filter evaluation
3. **Resource Isolation**: Each worker handles a subset of tenants with configurable limits
4. **Dynamic Scaling**: Kubernetes HPA scales workers based on load
5. **Tenant Affinity**: Consistent hashing keeps tenants on the same workers for cache efficiency

## Key Components

### 1. Block Cache Service

- Redis-based caching layer for blockchain RPC calls
- Prevents duplicate block fetches across workers
- Configurable TTL for blocks and latest block numbers

### 2. Tenant-Aware Repositories

- Implements OpenZeppelin Monitor's repository interfaces
- Loads configurations from PostgreSQL instead of files
- Provides multi-tenant isolation at the data layer

### 3. Worker Pool

- Manages multiple OpenZeppelin Monitor instances

- Each worker processes a subset of tenants
- Health checking and automatic tenant reloading

### 4. Shared Block Watcher

- Single instance per blockchain network
- Fetches blocks once and broadcasts to all workers
- Handles retry logic and error recovery

### 5. Load Balancer

- Distributes tenants across workers
- Supports multiple strategies:
  - Round-robin
  - Least loaded
  - Consistent hashing (default)
  - Activity-based

## Deployment

### Prerequisites

- Kubernetes cluster (1.26+)
- PostgreSQL database (from stellar-monitor-tenant-isolation)
- Redis cluster
- OpenZeppelin Monitor source code

### Quick Start

1. Build the orchestrator image:

```bash
docker build -t oz-monitor-orchestrator:latest .
```

1. Create namespace and secrets:

```bash
kubectl create namespace oz-monitor-orchestrator
kubectl -n oz-monitor-orchestrator create secret generic database-credentials \
  --from-literal=url="postgresql://user:pass@host:5432/tenant_isolation"
```

1. Deploy Redis:

```bash
kubectl apply -f k8s/redis-statefulset.yaml
```

1. Deploy the orchestrator:

```bash
kubectl apply -f k8s/worker-deployment.yaml
kubectl apply -f k8s/shared-block-watcher.yaml
kubectl apply -f k8s/hpa.yaml
```

1. Monitor scaling:

```bash
kubectl -n oz-monitor-orchestrator get hpa -w
```

## Configuration

The orchestrator can be configured via:

- Configuration file: `/etc/oz-monitor/config.yaml`
- Environment variables: `OZ_MONITOR_*`
- Command-line arguments

### Example Configuration

```yaml
database_url: "postgresql://user:password@localhost:5432/tenant_isolation"
redis_url: "redis://localhost:6379"

worker:
  max_tenants_per_worker: 50
  health_check_interval: 30s
  tenant_reload_interval: 5m

block_cache:
  block_ttl: 60
  latest_block_ttl: 5
  key_prefix: "oz_cache"

load_balancer:
  strategy: "consistent_hashing"
  rebalance_threshold: 0.2
  min_rebalance_interval: 5m
```

## Scaling Strategy

### Horizontal Scaling

The HPA automatically scales workers based on:

- CPU usage (70% threshold)
- Memory usage (80% threshold)
- Tenant count per worker (40 average)
- RPC rate per worker (100/s average)

### Performance Targets

- Block fetching: O(1) per network (shared)
- Filter evaluation: O(n) distributed across workers
- Tenant assignment: O(1) with consistent hashing
- Cache hit rate: >80% for active tenants

## Integration with Tenant Isolation API

The orchestrator reads tenant configurations from the stellar-monitor-tenant-isolation database:

```sql
-- Example query to get tenant monitors
SELECT m.*, t.max_monitors, t.max_rpc_requests_per_minute
FROM monitors m
JOIN tenants t ON m.tenant_id = t.id
WHERE t.is_active = true AND m.is_active = true;
```

## Monitoring

### Metrics

The orchestrator exposes Prometheus metrics on port 3000:

- `oz_monitor_worker_count`: Active workers
- `oz_monitor_tenant_count`: Tenants per worker
- `oz_monitor_rpc_calls_total`: RPC call rate
- `oz_monitor_cache_hits_total`: Cache hit rate
- `oz_monitor_block_processing_duration_seconds`: Processing latency

### Grafana Dashboard

Import the provided dashboard from `k8s/monitoring.yaml` to visualize:

- Worker distribution
- Tenant load balancing
- RPC usage by network
- Cache performance
- Block processing lag

## Development

### Running Locally

```bash
# Start dependencies
docker-compose up -d postgres redis

# Run worker mode
cargo run -- worker

# Run block watcher mode
cargo run -- block-watcher

# Run all services (development)
cargo run -- all
```

### Testing

```bash
cargo test
```

## Architecture Benefits

1. **No Core Modifications**: Uses OpenZeppelin Monitor as a library
2. **Linear Scaling**: Block fetching doesn't increase with tenant count
3. **Resource Efficiency**: Shared caching and block data
4. **Fault Isolation**: Tenant issues don't affect others
5. **Easy Upgrades**: Update OpenZeppelin Monitor dependency

## Future Enhancements

- [ ] Priority queue for high-value tenants
- [ ] GraphQL API for tenant management
- [ ] WebSocket support for real-time notifications
- [ ] Multi-region deployment support
- [ ] Cost attribution per tenant

## License

This orchestrator follows the same license as OpenZeppelin Monitor.
