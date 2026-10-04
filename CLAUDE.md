# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

Carbon Cache is a Rust (edition 2024) cache server with an HTTP API and a binary TCP protocol over a shared core. It runs as a single node (`CARBON_MODE=standalone`, default) or a Raft cluster (`CARBON_MODE=cluster`). [ARCHITECTURE.md](ARCHITECTURE.md) has detailed diagrams. [README.md](README.md) lists every `CARBON_*` env var; they are parsed in [shared/src/config.rs](shared/src/config.rs), and a `.env` file is loaded via dotenvy.

## Commands

```bash
cargo build                                  # what CI runs (.github/workflows/rust.yml)
cargo test                                   # what CI runs
cargo test -p carbon                         # one crate
cargo test -p carbon auth::password          # tests matching a path/name filter
cargo clippy --workspace                     # carbon/src/ports.rs has #![deny(clippy::all)]

cargo run -p carbon-server                   # standalone, HTTP :8080, TCP :5500
./scripts/start-standalone.sh                # release build + run standalone
./scripts/start-cluster.sh                   # release build, 3 nodes (HTTP 8080-8082, TCP 5500-5502, Raft 8091-8093), logs in /tmp/carbon-node-{1,2,3}.log
```

- `carbon-server` is the only real binary. The `server-http` and `server-tcp` `main.rs` files are stubs that just exit.
- Rust tests are inline `#[cfg(test)]` modules, mostly in `carbon/src/auth`, `carbon/src/persistence`, `storage-engine`, `server-tcp/src/protocol`, and `server-http` auth. `server-http/tests/main.rs` and `server-tcp/tests/integration_test.rs` are empty.
- The release profile keeps debug symbols (`debug = true`) for profiling. Load and profiling scripts and their docs are in `scripts/` (`LOAD_TESTING.md`, `TCP_PERFORMANCE_TESTING.md`).
- The default admin is `admin` / `admin123` (`CARBON_ADMIN_USERNAME` / `CARBON_ADMIN_PASSWORD`). Standalone state is written under `./data/.carbon/*.redb`. Cluster state is written to `./data/raft/{node_id}/raft.redb` and survives restarts. Delete these files to reset.

### Python client (`clients/carbon-client-python`, uv + hatchling)

```bash
cd clients/carbon-client-python
uv sync --extra dev
uv run pytest tests/test_protocol.py -v      # what the release CI runs
uv run pytest tests/test_client.py::TestX::test_y
```

`tests/conftest.py` has a session-scoped autouse fixture that **skips every test** unless a Carbon server is listening on localhost:5500 (TCP) and localhost:8080 (HTTP). Start `cargo run -p carbon-server` first. Pushing a tag matching `clients/python/v*` publishes to PyPI.

## Architecture

### Crate graph

`carbon-server` (entrypoint) → `server-http` (Axum), `server-tcp`, `carbon-raft` (openraft) → `carbon` (domain, auth, control/data planes) → `storage-engine` (Moka/Foyer) and `shared` (config). `shared-http` holds the HTTP request/response DTOs.

### The trait boundary is the central design

`server-http` and `server-tcp` only see trait objects. They never know which mode the server is in. Mode-specific wiring happens in exactly two files, [carbon-server/src/standalone.rs](carbon-server/src/standalone.rs) and [carbon-server/src/cluster.rs](carbon-server/src/cluster.rs). Each builds the same `server_http::AppState` and calls `server_tcp::process_connection` with different implementations behind these traits:

| Trait (in `carbon`) | Standalone | Cluster |
| --- | --- | --- |
| `CacheOperations<Vec<u8>, Bytes>` (`planes/data/operation.rs`) | `CacheOperationsService` | `RaftCacheNode` |
| `AdminOperations<Vec<u8>, Bytes>` (`planes/control/operation.rs`) | `CacheManager` (configs persisted in redb) | `RaftCacheNode` |
| `UserRepository` / `RoleRepository` (`auth/repository.rs`) | `RedbUserRepository` / `RedbRoleRepository` | `RaftUserRepository` / `RaftRoleRepository` (read the state machine) |

The third argument (`Option<RaftCacheNode>`) is `None` in standalone mode. HTTP and TCP handlers use it to decide whether to forward writes to the leader.

When adding an operation, change the trait and **both** implementations. In cluster mode a write also needs a new `RaftLogEntry` variant ([carbon-raft/src/types.rs](carbon-raft/src/types.rs)) and handling in `CacheStateMachine` ([carbon-raft/src/state_machine.rs](carbon-raft/src/state_machine.rs)).

### Code that is intentionally duplicated (keep in sync)

- Write forwarding to the Raft leader is implemented twice: [server-http/src/leader_proxy.rs](server-http/src/leader_proxy.rs) and [server-tcp/src/leader_proxy.rs](server-tcp/src/leader_proxy.rs).
- PUT/DELETE logic is implemented twice: the HTTP handlers in `server-http/src/handlers/cache/` and the TCP `do_put`/`do_delete` in [server-tcp/src/server.rs](server-tcp/src/server.rs).
- Each Redb repository has a Raft counterpart with the same method set.

### Storage backends

`storage-engine::UnifiedStorageFactory` implements the `StorageFactory` port ([carbon/src/ports.rs](carbon/src/ports.rs)) and creates one backend per cache from its `CacheConfig`:

- TTL / time-bound caches use Moka.
- Size-bounded caches use Foyer in memory, with optional disk overflow.
- A size-bounded config without `mem_bytes` returns `Error::InvalidArgument`.

Cache **configs** are persisted. Cache **values** live only in memory, except in cluster mode where they are replayed from the Raft log.

### Auth

- Every route except `/health`, `/auth/*`, `/raft/metrics`, `/cluster/nodes` and `/admin/ui` goes through `auth_middleware` ([server-http/src/routes.rs](server-http/src/routes.rs)). This includes the `/cache/...` data routes.
- The middleware tries a Bearer session token first. It falls back to HTTP Basic, which creates or reuses a session and returns it in the `X-Session-Token` / `X-Session-Reused` response headers.
- Permission checks (`check_permission` / `check_any_permission` against role `Permission`s) happen **inside each handler**, not at the router. New handlers must call them.
- Sessions (`MokaSessionRepository`, 1h TTL) are per-node and not replicated, even in cluster mode.
- Passwords are hashed with argon2.

### TCP protocol

Frames have a 4-byte big-endian length prefix (`LengthDelimitedCodec`, 8 MB max). Each payload starts with a 1-byte opcode, and encoding is done by hand with `bytes::Bytes` (no serde). The spec is in [server-tcp/PROTOCOL.md](server-tcp/PROTOCOL.md). If you change [server-tcp/src/protocol/mod.rs](server-tcp/src/protocol/mod.rs), update `clients/carbon-client-python/src/carbon_cache/_protocol.py` and its tests to match.

## Gotchas

- The README's Admin UI section refers to a `carbon-admin-ui` Dioxus crate that is not in the workspace. The router still serves `/admin/ui` from `target/dx/carbon-admin-ui/release/web/public`.
- `docs/superpowers/specs` and `docs/superpowers/plans` record past migrations (sled → redb, the standalone/cluster split) and a proposed Axum → Actix-Web migration for `server-http`. The code still uses Axum.
- Cluster nodes advertise `0.0.0.0:{port}` as their HTTP, TCP and Raft addresses in Raft membership (see `Config::from_env`).
