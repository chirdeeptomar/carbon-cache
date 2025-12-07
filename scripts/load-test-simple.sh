#!/bin/bash

# Simple load test with unique keys using curl
# No additional dependencies required

set -e

echo "=== Simple Load Test: Unique Keys Per Request ==="
echo ""

# Configuration
URL_BASE="http://localhost:8080/cache/test"
USER="admin"
PASS="admin123"
TOTAL_REQUESTS=1000
CONCURRENCY=50

echo "Configuration:"
echo "  URL:             $URL_BASE/[unique-key]"
echo "  Total requests:  $TOTAL_REQUESTS"
echo "  Concurrency:     $CONCURRENCY"
echo "  Auth:            Basic Auth ($USER)"
echo ""

# Check if server is running
if ! curl -s http://localhost:8080/health > /dev/null 2>&1; then
    echo "Error: Server is not running on localhost:8080"
    echo ""
    echo "Start the server with: cargo run --release --bin server-http"
    echo ""
    exit 1
fi

echo "Server is running. Starting load test..."
echo ""

# Create temporary directory for tracking
TMPDIR=$(mktemp -d)
SUCCESS_COUNT="$TMPDIR/success"
FAIL_COUNT="$TMPDIR/fail"
echo "0" > "$SUCCESS_COUNT"
echo "0" > "$FAIL_COUNT"

# Function to make a single request
make_request() {
    local key=$1
    local url="$URL_BASE/$key"

    if curl -s -u "$USER:$PASS" -X PUT "$url" \
        -H "Content-Type: application/json" \
        -d "{\"value\": \"test-value-$key\"}" \
        -o /dev/null -w "%{http_code}" 2>&1 | grep -q "^2"; then
        echo $(($(cat "$SUCCESS_COUNT") + 1)) > "$SUCCESS_COUNT"
    else
        echo $(($(cat "$FAIL_COUNT") + 1)) > "$FAIL_COUNT"
    fi
}

export -f make_request
export URL_BASE USER PASS SUCCESS_COUNT FAIL_COUNT

# Record start time
START=$(python3 -c 'import time; print(int(time.time() * 1000))')

# Run concurrent requests
seq 1 $TOTAL_REQUESTS | xargs -P $CONCURRENCY -I {} bash -c "make_request key-{}"

# Record end time
END=$(python3 -c 'import time; print(int(time.time() * 1000))')

# Calculate stats
DURATION=$((END - START))
SUCCESS=$(cat "$SUCCESS_COUNT")
FAILED=$(cat "$FAIL_COUNT")
RPS=$((TOTAL_REQUESTS * 1000 / DURATION))

echo ""
echo "=== Load Test Results ==="
echo "Duration:          $DURATION ms ($(echo "scale=2; $DURATION / 1000" | bc) seconds)"
echo "Total requests:    $TOTAL_REQUESTS"
echo "Successful:        $SUCCESS ($(echo "scale=2; $SUCCESS * 100 / $TOTAL_REQUESTS" | bc)%)"
echo "Failed:            $FAILED ($(echo "scale=2; $FAILED * 100 / $TOTAL_REQUESTS" | bc)%)"
echo "Throughput:        $RPS req/sec"
echo "Avg latency:       $(echo "scale=2; $DURATION / $TOTAL_REQUESTS" | bc) ms"
echo ""

# Cleanup
rm -rf "$TMPDIR"

echo "Note: Each request used a unique key (key-1, key-2, ..., key-$TOTAL_REQUESTS)"
echo "This tests realistic cache behavior with different keys."