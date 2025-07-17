#!/bin/bash

echo "=== System Verification ==="
echo

# Check services
echo "1. Service Status:"
echo -n "   - PostgreSQL: "
psql -U bhaven -d stellar_monitor_tenant -c 'SELECT 1' > /dev/null 2>&1 && echo "✓ Running" || echo "✗ Not running"

echo -n "   - Redis: "
redis-cli ping > /dev/null 2>&1 && echo "✓ Running" || echo "✗ Not running"

echo -n "   - Tenant Isolation API: "
curl -s http://localhost:3000/health > /dev/null 2>&1 && echo "✓ Running on port 3000" || echo "✗ Not running"

echo -n "   - Orchestrator: "
ps aux | grep -v grep | grep oz-monitor-orchestrator > /dev/null && echo "✓ Running" || echo "✗ Not running"

echo
echo "2. Database Contents:"
echo "   Checking tenant data..."
psql -U bhaven -d stellar_monitor_tenant -t -c "SELECT 'Tenants: ' || COUNT(*) FROM tenants UNION ALL SELECT 'Users: ' || COUNT(*) FROM users UNION ALL SELECT 'Networks: ' || COUNT(*) FROM tenant_networks UNION ALL SELECT 'Monitors: ' || COUNT(*) FROM tenant_monitors UNION ALL SELECT 'Triggers: ' || COUNT(*) FROM tenant_triggers;"

echo
echo "3. System Architecture:"
echo "   - API Layer: Stellar Monitor Tenant Isolation (Port 3000)"
echo "   - Orchestration: OZ Monitor Orchestrator (Port 3001)"
echo "   - Cache: Redis (Port 6379)"
echo "   - Database: PostgreSQL (Port 5432)"

echo
echo "4. Configuration:"
echo "   - Tenant: acme-corp"
echo "   - Monitor: Large USDC Transfers (> 10,000 USDC)"
echo "   - Network: Ethereum Mainnet"
echo "   - Triggers: Slack & Webhook notifications"

echo
echo "5. Process Information:"
ps aux | grep -E "(stellar-monitor-tenant|oz-monitor-orchestrator)" | grep -v grep

echo
echo "=== Verification Complete ==="