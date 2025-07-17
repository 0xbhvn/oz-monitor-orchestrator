#!/bin/bash

# Demo API calls for the multi-tenant monitoring system

echo "=== Demo API Calls ==="
echo

# 1. Login
echo "1. Login as admin@acme.com..."
LOGIN_RESPONSE=$(curl -s -X POST http://localhost:3000/api/v1/auth/login \
  -H "Content-Type: application/json" \
  -d '{"email":"admin@acme.com","password":"Test123!"}')

echo "Response: $LOGIN_RESPONSE"
TOKEN=$(echo $LOGIN_RESPONSE | grep -o '"access_token":"[^"]*' | cut -d'"' -f4)
echo "Token extracted: ${TOKEN:0:50}..."
echo

# 2. Get tenant info
echo "2. Getting tenant information..."
curl -s -X GET http://localhost:3000/api/v1/tenants/acme-corp \
  -H "Authorization: Bearer $TOKEN" | python3 -m json.tool
echo

# 3. List monitors
echo "3. Listing monitors for tenant..."
curl -s -X GET http://localhost:3000/api/v1/tenants/acme-corp/monitors \
  -H "Authorization: Bearer $TOKEN" | python3 -m json.tool
echo

# 4. List networks
echo "4. Listing networks for tenant..."
curl -s -X GET http://localhost:3000/api/v1/tenants/acme-corp/networks \
  -H "Authorization: Bearer $TOKEN" | python3 -m json.tool
echo

# 5. Check metrics
echo "5. Sample metrics from tenant isolation service..."
curl -s http://localhost:9090/metrics | head -10
echo

# 6. Check orchestrator status via API
echo "6. Checking orchestrator API..."
curl -s http://localhost:3001/api/v1/status 2>/dev/null || echo "Orchestrator status endpoint not available"
echo

echo "=== Demo Complete ==="
echo "The system is now monitoring for large USDC transfers on Ethereum mainnet."
echo "Any transfers over 10,000 USDC will trigger notifications."