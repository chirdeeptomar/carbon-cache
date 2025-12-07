#!/bin/bash

# Monitor server memory during load test
# Tracks RSS (Resident Set Size), VSZ (Virtual Memory Size), and CPU usage

set -e

# Try to find server process
SERVER_PID=$(pgrep -f "server-http" | head -1)

if [ -z "$SERVER_PID" ]; then
    echo "Error: Server not running"
    echo "Please start the server first: cargo run --release --bin server-http"
    exit 1
fi

echo "=== Memory Monitoring for server-http (PID: $SERVER_PID) ==="
echo ""
echo "Time(s)   RSS(MB)   VSZ(MB)   CPU%"
echo "-------   -------   -------   ----"

while true; do
    if ! ps -p $SERVER_PID > /dev/null 2>&1; then
        echo "Server process terminated"
        exit 1
    fi

    ps -p $SERVER_PID -o rss=,vsz=,%cpu= | awk -v t=$i '{printf "%-8d  %-8d  %-8d  %.1f\n", t, $1/1024, $2/1024, $3}'
    sleep 1
done

echo ""
echo "Monitoring complete!"
