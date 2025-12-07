#!/bin/bash

# High-performance load test with unique keys per request
# Requires: rust-script (install with: cargo install rust-script)

set -e

echo "=== Load Test Setup: Unique Keys Per Request ==="
echo ""

# Check if rust-script is installed
if ! command -v rust-script &> /dev/null; then
    echo "Error: rust-script is not installed"
    echo ""
    echo "Install with: cargo install rust-script"
    echo ""
    exit 1
fi

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

# Run the Rust script
rust-script scripts/load-test-unique-keys.rs "$@"