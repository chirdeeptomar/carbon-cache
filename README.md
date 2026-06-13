# Carbon Cache

Carbon Cache is a high-performance cache server written in Rust. It supports multiple eviction strategies (TTL, size-based, disk overflow) and exposes both an HTTP API and a TCP interface. Auth is built-in for all admin operations.

---

## Modes

Carbon runs in one of two modes, selected via the `CARBON_MODE` environment variable.

| Mode | `CARBON_MODE` | Use case |
| --- | --- | --- |
| **Standalone** | `standalone` (default) | Single-node, no consensus overhead |
| **Cluster** | `cluster` | Multi-node with Raft consensus |

---

## Standalone mode

Lightweight single-node server. No Raft. Cache configs and auth data are persisted to disk; cache values are in-memory only.

```bash
# Using the helper script (builds + starts)
./scripts/start-standalone.sh

# Or directly
CARBON_MODE=standalone cargo run -p carbon-server
```

Default ports: HTTP `8080`, TCP `5500`

### Standalone environment variables

| Variable | Default | Description |
| --- | --- | --- |
| `CARBON_HOST` | `localhost` | Bind address |
| `CARBON_HTTP_PORT` | `8080` | HTTP port |
| `CARBON_TCP_PORT` | `5500` | TCP port |
| `CARBON_DATA_DIR` | `./data` | Directory for auth and cache config persistence |
| `CARBON_ADMIN_USERNAME` | `admin` | Initial admin username |
| `CARBON_ADMIN_PASSWORD` | `admin123` | Initial admin password |

---

## Cluster mode

Multi-node server using Raft consensus. All writes are replicated across nodes. Any node can serve reads; writes are forwarded to the leader transparently.

```bash
# Using the helper script (starts a 3-node local cluster)
./scripts/start-cluster.sh

# Or start nodes individually:

# Node 1 — bootstraps the cluster
CARBON_MODE=cluster \
CARBON_NODE_ID=1 \
CARBON_HTTP_PORT=8080 \
CARBON_TCP_PORT=5500 \
CARBON_RAFT_PORT=8091 \
cargo run -p carbon-server

# Node 2 — joins via node 1
CARBON_MODE=cluster \
CARBON_NODE_ID=2 \
CARBON_HTTP_PORT=8081 \
CARBON_TCP_PORT=5501 \
CARBON_RAFT_PORT=8092 \
CARBON_SEEDS=localhost:8091 \
cargo run -p carbon-server
```

### Cluster environment variables

All standalone variables apply, plus:

| Variable | Default | Description |
| --- | --- | --- |
| `CARBON_NODE_ID` | `1` | Unique node ID (u64) |
| `CARBON_RAFT_PORT` | `8091` | Raft RPC port |
| `CARBON_SEEDS` | _(empty)_ | Comma-separated Raft addresses of existing nodes to join |
| `CARBON_CLUSTER_DATA_DIR` | `./data/raft/{node_id}` | Directory for Raft log and snapshot files |

---

## Connecting

### HTTP API

```bash
# Health check
curl http://localhost:8080/health

# Create a cache
curl -u admin:admin123 -X POST http://localhost:8080/admin/caches \
  -H 'Content-Type: application/json' \
  -d '{"name":"my-cache","eviction":"lru","max_capacity":10000}'

# Write a value
curl -X PUT http://localhost:8080/cache/my-cache/my-key \
  -H 'Content-Type: application/json' \
  -d '{"value":"hello"}'

# Read a value
curl http://localhost:8080/cache/my-cache/my-key
```

### TCP

Connect on port `5500` using the Carbon wire protocol.

---

## Admin UI

Build and embed the Admin UI into the HTTP server:

```bash
dx build --package carbon-admin-ui --release --verbose
```

> Builds a release version of the Admin UI and publishes it to the `public/` folder. The HTTP server serves it at `/admin/ui/`.

Run the Admin UI independently (dev mode):

```bash
dx serve -p carbon-admin-ui
```

> When running independently, set `CARBON_ALLOWED_ORIGINS` to allow the dev server's origin.

---

**Press `ctrl+c` to stop the server.**
