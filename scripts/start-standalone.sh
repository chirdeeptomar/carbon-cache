#!/usr/bin/env bash
# Start a single Carbon node in standalone mode (no Raft, in-memory cache).
#
# Ports: HTTP=8080  TCP=5500
#
# Auth data (users, roles) is persisted to CARBON_DATA_DIR (./data by default).
# Cache data is in-memory and lost on restart.
#
# Override any default with environment variables before running:
#
#   CARBON_HTTP_PORT=9090 CARBON_ADMIN_PASSWORD=secret ./scripts/start-standalone.sh
#
# Logs: stdout (CTRL+C to stop)

set -e

BINARY="./target/debug/carbon-server"

: "${CARBON_MODE:=standalone}"
: "${CARBON_HOST:=localhost}"
: "${CARBON_HTTP_PORT:=8080}"
: "${CARBON_TCP_PORT:=5500}"
: "${CARBON_DATA_DIR:=./data}"
: "${CARBON_ADMIN_USERNAME:=admin}"
: "${CARBON_ADMIN_PASSWORD:=admin123}"

export CARBON_MODE
export CARBON_HOST
export CARBON_HTTP_PORT
export CARBON_TCP_PORT
export CARBON_DATA_DIR
export CARBON_ADMIN_USERNAME
export CARBON_ADMIN_PASSWORD

echo "==> Building carbon-server (debug)..."
cargo build -p carbon-server

echo ""
echo "==> Starting Carbon in standalone mode"
echo "    HTTP : http://${CARBON_HOST}:${CARBON_HTTP_PORT}"
echo "    TCP  : ${CARBON_HOST}:${CARBON_TCP_PORT}"
echo "    Data : ${CARBON_DATA_DIR}"
echo "    Admin: ${CARBON_ADMIN_USERNAME}"
echo ""

exec "$BINARY"
