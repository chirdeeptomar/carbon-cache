#!/bin/bash

# High-performance load test: 1M requests/sec for 10 seconds
# Requires: oha (install with: brew install oha OR cargo install oha)

set -e

echo "=== Extreme Load Test: 1M req/sec for 10 seconds ==="
echo ""

# Check if oha is installed
if ! command -v oha &> /dev/null; then
    echo "Error: oha is not installed"
    echo ""
    echo "Install options:"
    echo "  1. Via Homebrew: brew install oha"
    echo "  2. Via Cargo: cargo install oha"
    echo ""
    exit 1
fi

# Server URL
URL="http://localhost:8080/cache/test/benchkey"

# Create Basic Auth header
AUTH_HEADER=$(echo -n "admin:admin123" | base64)

echo "Starting load test..."
echo "Target: 1,000,000 requests/second for 10 seconds"
echo "URL: $URL"
echo ""

# Run oha with 1M RPS target
oha \
  -p 2 \
  -z 1m \
  -q 100 \
  -c 1000 \
  -m PUT \
  -H "Authorization: Basic $AUTH_HEADER" \
  -H "Content-Type: application/json" \
  -d '{"value":"test"}' \
  "$URL"

echo ""
echo "Load test complete!"