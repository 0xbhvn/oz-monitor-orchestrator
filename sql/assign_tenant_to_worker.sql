-- Check if we have the tenant_worker_assignments table
-- If not, we need to manually tell the orchestrator about the tenant

-- First, let's see what tenant we have
SELECT 'Tenant info:' as info;
SELECT id, name, slug FROM tenants WHERE slug = 'acme-corp';

-- Get the tenant ID
SELECT 'Tenant monitors:' as info;
SELECT m.id, m.tenant_id, m.monitor_id, m.name, t.slug as tenant_slug
FROM tenant_monitors m
JOIN tenants t ON m.tenant_id = t.id;

-- Since the orchestrator doesn't have a proper API or database table for worker assignments,
-- we'll need to modify the code or use a different approach