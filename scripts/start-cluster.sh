#!/usr/bin/env bash
# Start a 3-node Carbon cluster on localhost for local testing.
#
# Node 1: HTTP=8080  TCP=5500  Raft=8091  (first to start, becomes leader)
# Node 2: HTTP=8081  TCP=5501  Raft=8092  (seeds=all three raft ports)
# Node 3: HTTP=8082  TCP=5502  Raft=8093  (seeds=all three raft ports)
#
# User data (caches, users, roles) is preserved across restarts — it lives in
# ./data/raft/{1,2,3}/raft.redb. Only the cluster membership is re-established.
#
# Clients can connect to any node: all reads are local, writes are forwarded
# to the leader transparently. On startup, query /cluster/nodes on any node
# to discover the full list of addresses.
#
# Logs: /tmp/carbon-node-{1,2,3}.log

set -e

BINARY="./target/release/carbon-server"
ALL_SEEDS="localhost:8091,localhost:8092,localhost:8093"

echo "==> Building carbon-server (release)..."
cargo build -p carbon-server --release

echo "==> Starting 3-node cluster..."

for i in 1 2 3; do
  HTTP_PORT=$((8079 + i))   # 8080, 8081, 8082
  TCP_PORT=$((5499  + i))   # 5500, 5501, 5502
  RAFT_PORT=$((8090 + i))   # 8091, 8092, 8093
  LOG="/tmp/carbon-node-${i}.log"

  SEEDS=""
  if [ "$i" -gt 1 ]; then
    SEEDS="$ALL_SEEDS"
  fi

  CARBON_MODE=cluster \
  CARBON_NODE_ID=$i \
  CARBON_HTTP_PORT=$HTTP_PORT \
  CARBON_TCP_PORT=$TCP_PORT \
  CARBON_RAFT_PORT=$RAFT_PORT \
  CARBON_SEEDS="$SEEDS" \
  CARBON_HOST=0.0.0.0 \
    "$BINARY" >"$LOG" 2>&1 &

  echo "    Node $i started (HTTP=:$HTTP_PORT  TCP=:$TCP_PORT  Raft=:$RAFT_PORT) — log: $LOG"
  sleep 2     # give each node time to join before the next one starts
done

echo ""
echo "==> Cluster started. Waiting 4s for leader election..."
sleep 4

echo ""
echo "==> Cluster nodes:"
for port in 8080 8081 8082; do
  result=$(curl -s "http://localhost:${port}/cluster/nodes" 2>/dev/null)
  if [ -n "$result" ]; then
    echo "$result" | python3 -m json.tool 2>/dev/null
    break
  fi
done || echo "  (no node ready yet — check /tmp/carbon-node-*.log)"

echo ""
echo "==> Raft metrics:"
for port in 8080 8081 8082; do
  result=$(curl -s "http://localhost:${port}/raft/metrics" 2>/dev/null)
  if [ -n "$result" ]; then
    echo "$result" | python3 -m json.tool 2>/dev/null
    break
  fi
done || echo "  (not ready)"

echo ""
echo "Cluster running. Press Ctrl+C to stop all nodes."
echo "  Logs               : /tmp/carbon-node-{1,2,3}.log"
echo "  HTTP               : http://localhost:8080  http://localhost:8081  http://localhost:8082"
echo "  TCP                : localhost:5500  localhost:5501  localhost:5502"
echo "  Raft RPC (internal): localhost:8091  localhost:8092  localhost:8093"
# Wait and then stop everything on Ctrl+C
trap 'echo ""; echo "Stopping cluster..."; pkill -f carbon-server 2>/dev/null; exit 0' INT TERM
wait
