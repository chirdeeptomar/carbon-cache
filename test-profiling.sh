#!/bin/bash

echo "Starting server with dhat profiling..."

# Start server in background
cargo run --release &
SERVER_PID=$!

echo "Server PID: $SERVER_PID"
echo "Waiting for server to start..."
sleep 3

# Create a cache
echo "Creating test cache..."
curl -s -u admin:admin123 -X POST http://localhost:8080/admin/caches \
  -H "Content-Type: application/json" \
  -d '{
    "name": "test",
    "eviction": "ttl",
    "mem_bytes": 1048576
  }' | jq .

# Make some PUT requests
echo ""
echo "Making PUT requests..."
for i in {1..100}; do
  curl -s -u admin:admin123 -X PUT "http://localhost:8080/cache/test/key$i" \
    -H "Content-Type: application/json" \
    -d "{\"value\":\"This is test value $i with some data to make it bigger\"}" > /dev/null
done
echo "PUT requests complete"

# Make some GET requests
echo "Making GET requests..."
for i in {1..100}; do
  curl -s -u admin:admin123 -X GET "http://localhost:8080/cache/test/key$i" > /dev/null
done
echo "GET requests complete"

# Give server a moment
sleep 1

# Shutdown server gracefully (Ctrl+C)
echo ""
echo "Sending Ctrl+C to server..."
kill -INT $SERVER_PID

# Wait for graceful shutdown
echo "Waiting for graceful shutdown..."
sleep 3

# Check if dhat-heap.json was created
if [ -f "dhat-heap.json" ]; then
    echo ""
    echo "✓ Success! dhat-heap.json created"
    echo "File size: $(ls -lh dhat-heap.json | awk '{print $5}')"
    echo ""
    echo "View results at: https://nnethercote.github.io/dh_view/dh_view.html"
    echo "Then drag and drop dhat-heap.json onto the page"
else
    echo ""
    echo "✗ Error: dhat-heap.json not found"
    echo "Check server logs above for errors"
fi
