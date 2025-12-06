#!/bin/bash

echo "Testing transparent session management..."
echo "=========================================="
echo ""

# Start server in background
echo "Starting server..."
cargo run --release > /tmp/server.log 2>&1 &
SERVER_PID=$!
echo "Waiting for server to start (PID: $SERVER_PID)..."
sleep 8

# Create a test cache first
echo "Creating test cache..."
curl -s -u admin:admin123 -X POST http://localhost:8080/admin/caches \
  -H "Content-Type: application/json" \
  -d '{"name": "test", "eviction": "ttl", "mem_bytes": 1048576}' > /dev/null 2>&1

echo ""
echo "Making first request with Basic Auth (should create new session)..."
RESPONSE1=$(curl -s -i -u admin:admin123 -X PUT "http://localhost:8080/cache/test/key1" \
  -H "Content-Type: application/json" -d '{"value":"test1"}' 2>&1)
TOKEN1=$(echo "$RESPONSE1" | grep -i "x-session-token:" | head -1 | cut -d' ' -f2 | tr -d '\r\n')
REUSED1=$(echo "$RESPONSE1" | grep -i "x-session-reused:" | head -1 | cut -d' ' -f2 | tr -d '\r\n')

echo "  x-session-token: $TOKEN1"
echo "  x-session-reused: $REUSED1"
echo ""

sleep 1

echo "Making second request with same Basic Auth (should reuse session)..."
RESPONSE2=$(curl -s -i -u admin:admin123 -X GET "http://localhost:8080/cache/test/key1" 2>&1)
TOKEN2=$(echo "$RESPONSE2" | grep -i "x-session-token:" | head -1 | cut -d' ' -f2 | tr -d '\r\n')
REUSED2=$(echo "$RESPONSE2" | grep -i "x-session-reused:" | head -1 | cut -d' ' -f2 | tr -d '\r\n')

echo "  x-session-token: $TOKEN2"
echo "  x-session-reused: $REUSED2"
echo ""

# Verify session reuse
if [ "$TOKEN1" == "$TOKEN2" ] && [ -n "$TOKEN1" ]; then
    echo "✓ SUCCESS: Session was reused (same token)"
else
    echo "✗ FAIL: Different tokens received or empty token"
    echo "  Token 1: $TOKEN1"
    echo "  Token 2: $TOKEN2"
fi

if [ "$REUSED2" == "true" ]; then
    echo "✓ SUCCESS: x-session-reused header indicates session was reused"
else
    echo "✗ FAIL: x-session-reused header should be 'true' but got: $REUSED2"
fi

echo ""
echo "Making third request with Bearer token..."
RESPONSE3=$(curl -s -i -H "Authorization: Bearer $TOKEN1" -X GET "http://localhost:8080/cache/test/key1" 2>&1)
BEARER_STATUS=$(echo "$RESPONSE3" | grep "HTTP" | awk '{print $2}')

if [ "$BEARER_STATUS" == "200" ]; then
    echo "✓ SUCCESS: Bearer token authentication works"
else
    echo "✗ FAIL: Bearer token authentication failed (status: $BEARER_STATUS)"
fi

echo ""
echo "Shutting down server..."
kill -INT $SERVER_PID 2>/dev/null
sleep 2

echo "Test complete!"
