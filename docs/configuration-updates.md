# Configuration Update Behavior

This document explains how configuration updates are handled in the OZ Monitor Orchestrator and provides guidance on managing configuration changes in production environments.

## Overview

The OZ Monitor Orchestrator loads tenant configurations (monitors, triggers, filters) from PostgreSQL when workers start. Currently, **configuration updates are NOT automatically fetched for the next block without restarting services**.

## Current Architecture

### Configuration Loading Flow

1. **Initial Load**: When a worker starts, it loads all configurations for its assigned tenants
2. **Caching**: Configurations are cached in memory using `DashMap` structures
3. **Processing**: All block processing uses the cached configurations
4. **No Auto-Refresh**: Cache is not automatically invalidated or refreshed

### Key Components

```rust
// In OzMonitorServices
monitor_cache: Arc<DashMap<Uuid, HashMap<String, Monitor>>>,
contract_spec_cache: Arc<DashMap<String, ContractSpec>>,
```

### Configuration Sources

Configurations are loaded from PostgreSQL tables:

- `tenant_monitors`: Monitor definitions and filter rules
- `tenant_networks`: Network configurations and RPC endpoints  
- `tenant_triggers`: Trigger definitions and notification settings
- `trigger_scripts`: Custom trigger scripts

## Current Limitations

### 1. No Automatic Refresh

- Workers continue using cached configurations indefinitely
- New monitors/triggers won't be picked up
- Modified filters won't take effect
- Deleted configurations remain active in memory

### 2. Incomplete Reload Implementation

The worker pool has a `tenant_reload_interval` (default 5 minutes) but the reload task only changes status without actually reloading configurations:

```rust
// Current implementation - doesn't actually reload
async fn start_tenant_reload(&self) -> tokio::task::JoinHandle<()> {
    // ...
    loop {
        interval.tick().await;
        *status.write().await = WorkerStatus::Reloading;
        // TODO: Actual reload logic would go here
        *status.write().await = WorkerStatus::Running;
    }
}
```

### 3. Cache Invalidation

- No mechanism to detect database changes
- No TTL on cached configurations
- No versioning to track configuration updates

## Available Workarounds

### 1. Worker Restart

The most reliable method to pick up configuration changes:

```bash
# Restart a specific worker pod in Kubernetes
kubectl delete pod worker-deployment-xxxxx -n oz-monitor-orchestrator

# Or perform a rolling update
kubectl rollout restart deployment/worker-deployment -n oz-monitor-orchestrator
```

### 2. Manual Reload (If Implemented)

The `OzMonitorServices` has a `reload_configurations()` method that can be called:

```rust
// Clear cache and reload for specific tenants
oz_services.reload_configurations(&tenant_ids).await?;
```

This could be exposed via an admin API endpoint.

### 3. Tenant Reassignment

Reassigning tenants to a worker triggers a configuration reload:

```rust
// Via the worker pool manager
worker_pool.reassign_tenants(worker_id, new_tenant_ids).await?;
```

## Implementation Roadmap

### Short-term: Fix Reload Task

Update the tenant reload task to actually reload configurations:

```rust
async fn start_tenant_reload(&self) -> tokio::task::JoinHandle<()> {
    let oz_services = self.oz_services.clone();
    let tenant_ids = self.assigned_tenants.clone();
    let interval = self.config.tenant_reload_interval;
    
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(interval);
        loop {
            interval.tick().await;
            let tenants = tenant_ids.read().await.clone();
            if let Some(services) = &oz_services {
                let _ = services.reload_configurations(&tenants).await;
            }
        }
    })
}
```

### Medium-term: Database Change Notifications

Implement PostgreSQL LISTEN/NOTIFY for real-time updates:

```sql
-- Trigger on configuration changes
CREATE OR REPLACE FUNCTION notify_config_change()
RETURNS TRIGGER AS $$
BEGIN
    PERFORM pg_notify('config_change', 
        json_build_object(
            'tenant_id', NEW.tenant_id,
            'table', TG_TABLE_NAME,
            'operation', TG_OP
        )::text
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Apply to relevant tables
CREATE TRIGGER monitor_change_notify 
    AFTER INSERT OR UPDATE OR DELETE ON tenant_monitors
    FOR EACH ROW EXECUTE FUNCTION notify_config_change();
```

### Long-term: Advanced Cache Management

1. **Configuration Versioning**
   - Add version column to configuration tables
   - Track loaded version in cache
   - Periodic version check for updates

2. **Lazy Loading with TTL**
   - Set expiration on cached entries
   - Reload on cache miss
   - Background refresh before expiration

3. **Differential Updates**
   - Load only changed configurations
   - Minimize database queries
   - Reduce memory churn

## Best Practices

### For Operators

1. **Plan Configuration Updates**
   - Batch configuration changes together
   - Schedule updates during low-traffic periods
   - Use rolling updates for zero downtime

2. **Monitor Worker Health**
   - Watch for configuration drift
   - Monitor worker restart frequency
   - Set up alerts for reload failures

3. **Test Configuration Changes**
   - Validate configurations before applying
   - Test in staging environment first
   - Have rollback procedures ready

### For Developers

1. **Make Configurations Backward Compatible**
   - Support old and new formats during transition
   - Validate configurations on load
   - Log configuration changes

2. **Design for Eventual Consistency**
   - Accept that workers may have different configs briefly
   - Make features tolerant of configuration lag
   - Avoid breaking changes

3. **Implement Health Checks**
   - Include configuration version in health status
   - Expose last reload timestamp
   - Alert on stale configurations

## Configuration Update Scenarios

### Adding a New Monitor

1. Insert configuration into `tenant_monitors` table
2. Workers continue with existing monitors
3. Restart workers or wait for reload implementation
4. New monitor becomes active

### Modifying Filter Rules

1. Update configuration in database
2. Running workers use old filters
3. Blocks may be processed with outdated rules
4. After restart, new filters apply to all blocks

### Removing a Monitor

1. Set `is_active = false` in database
2. Monitor remains active in worker memory
3. Continues processing until restart
4. Important: Use soft deletes for safety

## Monitoring and Observability

### Metrics to Track

- Configuration load timestamp per worker
- Cache hit/miss rates
- Reload frequency and duration
- Configuration version per tenant

### Useful Queries

```sql
-- Check latest configuration updates
SELECT 
    tenant_id,
    COUNT(*) as config_count,
    MAX(updated_at) as last_update
FROM tenant_monitors
WHERE is_active = true
GROUP BY tenant_id
ORDER BY last_update DESC;

-- Find recently modified configurations
SELECT 
    tenant_id,
    monitor_id,
    name,
    updated_at
FROM tenant_monitors
WHERE updated_at > NOW() - INTERVAL '1 hour'
ORDER BY updated_at DESC;
```

## Conclusion

While the current implementation requires service restarts for configuration updates, the architecture supports adding dynamic reloading capabilities. Understanding these limitations helps operators plan configuration changes effectively and developers implement improvements systematically.

For production deployments, use Kubernetes rolling updates to ensure configuration changes are applied with zero downtime. Future enhancements will enable real-time configuration updates without service disruption.
