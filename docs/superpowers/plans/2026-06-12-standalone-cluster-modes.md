# Standalone / Cluster Mode Split Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Split carbon-server into two fully independent bootstrap paths — `standalone` (lightweight, in-memory, no Raft) and `cluster` (Raft consensus) — selected at startup via `CARBON_MODE` env var.

**Architecture:** Add a `ServerMode` enum to `shared::config::Config`. `carbon-server/src/main.rs` reads the mode and delegates to either `standalone::start()` or `cluster::start()`, each a self-contained module. `server-http::AppState` and `server-tcp::process_connection` are generalised to accept `Arc<dyn CacheOperations<Vec<u8>, Bytes>>` instead of `Arc<RaftCacheNode>`, so both modes can reuse the same HTTP/TCP servers.

**Tech Stack:** Rust, openraft 0.9, redb 4, axum 0.8, tokio 1.52

---

## File Map

| File | Change |
|------|--------|
| `shared/src/config.rs` | Add `ServerMode` enum + `mode` field to `Config` |
| `server-http/src/state.rs` | Replace `raft_node: Arc<RaftCacheNode>` with `cache_ops: Arc<dyn CacheOperations<…>>` + `raft_node: Option<Arc<RaftCacheNode>>` |
| `server-http/src/handlers/cache/basic.rs` | Use `state.cache_ops` for put/get/delete; leader-proxy guarded by `state.raft_node` |
| `server-http/src/handlers/admin/cache.rs` | Use `state.raft_node.as_ref().expect(…)` for Raft writes; reads from `state.cache_ops` indirectly via state machine |
| `server-http/src/handlers/admin/users.rs` | Use `state.raft_node.as_ref().expect(…)` |
| `server-http/src/handlers/admin/roles.rs` | Use `state.raft_node.as_ref().expect(…)` |
| `server-http/src/handlers/raft.rs` | Use `state.raft_node.as_ref().expect(…)` |
| `server-http/src/leader_proxy.rs` | Accept `&Arc<RaftCacheNode>` (unchanged) |
| `server-tcp/src/server.rs` | Change `process_connection` signature to `Arc<dyn CacheOperations<Vec<u8>, Bytes>>` |
| `server-tcp/src/lib.rs` | Re-export updated `process_connection` |
| `carbon-server/src/main.rs` | Read mode, delegate to `standalone::start` or `cluster::start` |
| `carbon-server/src/standalone.rs` | **New** — full standalone bootstrap (CacheManager + RedbRepository) |
| `carbon-server/src/cluster.rs` | **New** — full cluster bootstrap (current main.rs logic) |

---

## Task 1: Add `ServerMode` to `Config`

**Files:**
- Modify: `shared/src/config.rs`

- [ ] **Step 1: Add `ServerMode` enum and `mode` field**

In `shared/src/config.rs`, add before the `Config` struct:

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum ServerMode {
    Standalone,
    Cluster,
}
```

Add `pub mode: ServerMode,` as the first field of `Config`.

- [ ] **Step 2: Parse `CARBON_MODE` in `Config::from_env`**

At the top of the `from_env()` body, before any other `let` bindings, add:

```rust
let mode = match std::env::var("CARBON_MODE")
    .unwrap_or_else(|_| "standalone".to_string())
    .to_lowercase()
    .as_str()
{
    "cluster" => ServerMode::Cluster,
    _ => ServerMode::Standalone,
};
```

Then add `mode,` as the first field in the `Self { … }` struct literal.

- [ ] **Step 3: Verify it compiles**

```bash
cd /Users/chirdeeptomar/opensource/carbon && cargo check -p shared 2>&1
```

Expected: no errors.

- [ ] **Step 4: Commit**

```bash
git add shared/src/config.rs
git commit -m "feat(shared): add ServerMode enum and CARBON_MODE config field"
```

---

## Task 2: Generalise `server-tcp` — remove `RaftCacheNode` dependency

The TCP server already uses the `CacheOperations` trait internally (`let ops: &dyn CacheOperations<…> = &*raft_node`). We just need to accept the trait object directly and remove the Raft-specific leader-forwarding (standalone has no leader concept).

**Files:**
- Modify: `server-tcp/src/server.rs`
- Modify: `server-tcp/src/lib.rs`
- Modify: `server-tcp/Cargo.toml`

- [ ] **Step 1: Rewrite `server-tcp/src/server.rs`**

Replace the entire file with:

```rust
use crate::protocol::{Request, Response};
use bytes::Bytes;
use carbon::planes::data::operation::CacheOperations;
use futures::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio_util::codec::{Framed, LengthDelimitedCodec};
use tracing::info;

fn make_codec() -> LengthDelimitedCodec {
    LengthDelimitedCodec::builder()
        .length_field_length(4)
        .max_frame_length(8 * 1024 * 1024)
        .new_codec()
}

pub async fn process_connection(
    socket: TcpStream,
    ops: Arc<dyn CacheOperations<Vec<u8>, Bytes>>,
) -> Result<(), Box<dyn std::error::Error>> {
    socket.set_nodelay(true).ok();

    let mut framed = Framed::new(socket, make_codec());

    while let Some(frame_result) = framed.next().await {
        let frame = frame_result?;

        let request = match Request::decode(frame.freeze()) {
            Ok(req) => req,
            Err(e) => {
                tracing::error!("Failed to decode request: {}", e);
                framed.send(Response::Error { msg: e }.encode()).await?;
                continue;
            }
        };

        info!("Received request: {:?}", request);

        let response = match request {
            Request::Ping => Response::Pong,

            Request::Put { cache_name, key, value } => {
                match ops.put(&cache_name, key.to_vec(), value).await {
                    Ok(_) => Response::Ok,
                    Err(shared::Error::CacheNotFound(name)) => {
                        Response::Error { msg: format!("Cache not found: {name}") }
                    }
                    Err(e) => Response::Error { msg: format!("Put failed: {e}") },
                }
            }

            Request::Get { cache_name, key } => {
                match ops.get(&cache_name, &key.to_vec()).await {
                    Ok(r) if r.found => Response::Value { value: r.message },
                    Ok(_) => Response::NotFound,
                    Err(shared::Error::CacheNotFound(name)) => {
                        Response::Error { msg: format!("Cache not found: {name}") }
                    }
                    Err(e) => Response::Error { msg: format!("Get failed: {e}") },
                }
            }

            Request::Delete { cache_name, key } => {
                match ops.delete(&cache_name, &key.to_vec()).await {
                    Ok(_) => Response::Ok,
                    Err(shared::Error::CacheNotFound(name)) => {
                        Response::Error { msg: format!("Cache not found: {name}") }
                    }
                    Err(e) => Response::Error { msg: format!("Delete failed: {e}") },
                }
            }
        };

        framed.send(response.encode()).await?;
    }

    Ok(())
}
```

Note: leader-forwarding is removed. In cluster mode, `carbon-server/cluster.rs` will wrap the `RaftCacheNode` in an `Arc<dyn CacheOperations>` — the node already implements the trait. Leader forwarding for the TCP path will be handled at the cluster bootstrap level if needed (out of scope for this plan; the Raft node's `CacheOperations` impl already routes writes to leader internally).

- [ ] **Step 2: Update `server-tcp/src/lib.rs`** — no change needed to the re-export, but verify `carbon-raft` is no longer imported directly.

Check:
```bash
grep -n "carbon.raft\|RaftCacheNode" /Users/chirdeeptomar/opensource/carbon/server-tcp/src/server.rs
```
Expected: no output.

- [ ] **Step 3: Remove `carbon-raft` from `server-tcp/Cargo.toml`**

In `server-tcp/Cargo.toml`, delete the line:
```
carbon-raft.workspace = true
```

- [ ] **Step 4: Compile**

```bash
cargo check -p server-tcp 2>&1
```

Expected: no errors.

- [ ] **Step 5: Commit**

```bash
git add server-tcp/src/server.rs server-tcp/Cargo.toml
git commit -m "refactor(server-tcp): accept CacheOperations trait object, remove RaftCacheNode dep"
```

---

## Task 3: Generalise `server-http` AppState

**Files:**
- Modify: `server-http/src/state.rs`

- [ ] **Step 1: Rewrite `server-http/src/state.rs`**

Replace the entire file with:

```rust
use bytes::Bytes;
use carbon::auth::{AuthService, MokaSessionRepository, RoleService, SessionStore, UserService};
use carbon::events::CacheItemEvent;
use carbon::planes::data::operation::CacheOperations;
use carbon_raft::node::RaftCacheNode;
use std::sync::Arc;
use tokio::sync::broadcast;

/// Server state shared across all HTTP handlers.
///
/// `cache_ops` is the active implementation of cache reads/writes — either
/// a `CacheOperationsService` (standalone) or `RaftCacheNode` (cluster).
///
/// `raft_node` is `Some` only in cluster mode and is required by Raft-specific
/// handlers (metrics, membership, user/role mutations via consensus).
#[derive(Clone)]
pub struct AppState {
    pub event_channel: broadcast::Sender<CacheItemEvent>,
    pub auth_service: Arc<AuthService>,
    pub user_service: Arc<UserService>,
    pub role_service: Arc<RoleService>,
    pub session_store: Arc<SessionStore<MokaSessionRepository>>,
    pub cache_ops: Arc<dyn CacheOperations<Vec<u8>, Bytes>>,
    pub raft_node: Option<Arc<RaftCacheNode>>,
}

impl AppState {
    pub fn new(
        cache_ops: Arc<dyn CacheOperations<Vec<u8>, Bytes>>,
        raft_node: Option<Arc<RaftCacheNode>>,
        auth_service: Arc<AuthService>,
        user_service: Arc<UserService>,
        role_service: Arc<RoleService>,
        session_store: Arc<SessionStore<MokaSessionRepository>>,
    ) -> Self {
        let (event_tx, _event_rx) = broadcast::channel::<CacheItemEvent>(1000);
        Self {
            event_channel: event_tx,
            auth_service,
            user_service,
            role_service,
            session_store,
            cache_ops,
            raft_node,
        }
    }
}
```

- [ ] **Step 2: Verify it compiles in isolation**

```bash
cargo check -p server-http 2>&1 | head -40
```

Expected: errors only in handlers (they still reference `state.raft_node` directly) — that's expected; we fix them next.

- [ ] **Step 3: Commit state.rs before handler changes**

```bash
git add server-http/src/state.rs
git commit -m "refactor(server-http): generalise AppState to CacheOperations trait + Option<RaftCacheNode>"
```

---

## Task 4: Update cache handlers to use `state.cache_ops`

**Files:**
- Modify: `server-http/src/handlers/cache/basic.rs`
- Modify: `server-http/src/leader_proxy.rs`

The three cache handlers (`put_value`, `get_value`, `delete_value`) currently do `let raft = &state.raft_node` and call `raft.is_leader()` before forwarding. In cluster mode we still need leader-forwarding; in standalone mode `raft_node` is `None` so we skip it.

- [ ] **Step 1: Rewrite `server-http/src/handlers/cache/basic.rs`**

```rust
use crate::leader_proxy::forward_to_leader;
use crate::state::AppState;
use axum::{
    extract::{Path, Request, State},
    http::StatusCode,
    Json,
};
use bytes::Bytes;
use carbon::planes::data::operation::CacheOperations;
use shared_http::api::{DeleteResponse, GetResponse, PutRequest, PutResponse};
use tracing::info;

/// PUT /cache/:cache_name/:key
pub async fn put_value(
    State(state): State<AppState>,
    Path((cache_name, key)): Path<(String, String)>,
    req: Request,
) -> Result<Json<PutResponse>, StatusCode> {
    info!("PUT: cache={}, key={}", cache_name, key);

    // In cluster mode, forward writes to the leader.
    if let Some(raft) = &state.raft_node {
        if !raft.is_leader() {
            let resp = forward_to_leader(raft, req).await;
            let status = resp.status();
            if status.is_success() {
                let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
                    .await
                    .map_err(|_| StatusCode::BAD_GATEWAY)?;
                let put_resp: PutResponse =
                    serde_json::from_slice(&body).map_err(|_| StatusCode::BAD_GATEWAY)?;
                return Ok(Json(put_resp));
            } else {
                return Err(StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY));
            }
        }
    }

    let body = axum::body::to_bytes(req.into_body(), usize::MAX)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    let req_body: PutRequest =
        serde_json::from_slice(&body).map_err(|_| StatusCode::BAD_REQUEST)?;

    let key_bytes = key.into_bytes();
    let value = Bytes::from(req_body.value);

    match state.cache_ops.put(&cache_name, key_bytes, value).await {
        Ok(_) => Ok(Json(PutResponse { ok: true })),
        Err(shared::Error::CacheNotFound(_)) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// GET /cache/:cache_name/:key
pub async fn get_value(
    State(state): State<AppState>,
    Path((cache_name, key)): Path<(String, String)>,
) -> Result<Json<GetResponse>, StatusCode> {
    info!("GET: cache={}, key={}", cache_name, key);

    let key_bytes = key.into_bytes();

    match state.cache_ops.get(&cache_name, &key_bytes).await {
        Ok(result) => {
            let value = String::from_utf8(result.message.to_vec())
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            Ok(Json(GetResponse { found: result.found, value, ttl_ms_remaining: 0 }))
        }
        Err(shared::Error::NotFound) => {
            Ok(Json(GetResponse { found: false, value: String::new(), ttl_ms_remaining: 0 }))
        }
        Err(shared::Error::CacheNotFound(_)) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// DELETE /cache/:cache_name/:key
pub async fn delete_value(
    State(state): State<AppState>,
    Path((cache_name, key)): Path<(String, String)>,
    req: Request,
) -> Result<Json<DeleteResponse>, StatusCode> {
    info!("DELETE: cache={}, key={}", cache_name, key);

    // In cluster mode, forward deletes to the leader.
    if let Some(raft) = &state.raft_node {
        if !raft.is_leader() {
            let resp = forward_to_leader(raft, req).await;
            let status = resp.status();
            if status.is_success() {
                let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
                    .await
                    .map_err(|_| StatusCode::BAD_GATEWAY)?;
                let del_resp: DeleteResponse =
                    serde_json::from_slice(&body).map_err(|_| StatusCode::BAD_GATEWAY)?;
                return Ok(Json(del_resp));
            } else {
                return Err(StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY));
            }
        }
    }

    let key_bytes = key.into_bytes();

    match state.cache_ops.delete(&cache_name, &key_bytes).await {
        Ok(result) => Ok(Json(DeleteResponse { deleted: result.deleted })),
        Err(shared::Error::CacheNotFound(_)) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}
```

- [ ] **Step 2: Compile**

```bash
cargo check -p server-http 2>&1 | grep "^error" | head -20
```

Expected: errors only in `admin/cache.rs`, `admin/users.rs`, `admin/roles.rs`, `handlers/raft.rs` — we fix those next.

- [ ] **Step 3: Commit**

```bash
git add server-http/src/handlers/cache/basic.rs
git commit -m "refactor(server-http/cache): use state.cache_ops, guard leader-proxy behind Option<RaftCacheNode>"
```

---

## Task 5: Update admin/cache handlers

**Files:**
- Modify: `server-http/src/handlers/admin/cache.rs`

All mutations go through `state.raft_node.as_ref().expect(…)`. The reads (`list_caches`, `describe_cache`) access the Raft state machine directly — these also only make sense in cluster mode, so they stay Raft-gated.

- [ ] **Step 1: Replace all `&state.raft_node` with `state.raft_node.as_ref().expect("admin/cache routes require cluster mode")`**

In `server-http/src/handlers/admin/cache.rs`:

Change every occurrence of:
```rust
let raft = &state.raft_node;
```
to:
```rust
let raft = state.raft_node.as_ref().expect("admin/cache routes require cluster mode");
```

Also change the two direct `state.raft_node.state.read().await` accesses in `list_caches` and `describe_cache`:
```rust
// was:
let sm = state.raft_node.state.read().await;
// becomes:
let sm = state.raft_node.as_ref().expect("list_caches requires cluster mode").state.read().await;
```

- [ ] **Step 2: Compile**

```bash
cargo check -p server-http 2>&1 | grep "^error" | head -20
```

- [ ] **Step 3: Commit**

```bash
git add server-http/src/handlers/admin/cache.rs
git commit -m "refactor(server-http/admin/cache): use Option<RaftCacheNode> via expect"
```

---

## Task 6: Update admin/users and admin/roles handlers

**Files:**
- Modify: `server-http/src/handlers/admin/users.rs`
- Modify: `server-http/src/handlers/admin/roles.rs`

- [ ] **Step 1: Update `admin/users.rs`**

Replace every occurrence of:
```rust
let raft = &state.raft_node;
```
with:
```rust
let raft = state.raft_node.as_ref().expect("admin/users routes require cluster mode");
```

Also replace:
```rust
state.raft_node.list_users()
```
with:
```rust
state.raft_node.as_ref().expect("list_users requires cluster mode").list_users()
```

And:
```rust
state.raft_node.get_user_by_username(&username)
```
with:
```rust
state.raft_node.as_ref().expect("get_user requires cluster mode").get_user_by_username(&username)
```

- [ ] **Step 2: Update `admin/roles.rs`**

Replace every occurrence of:
```rust
let raft = &state.raft_node;
```
with:
```rust
let raft = state.raft_node.as_ref().expect("admin/roles routes require cluster mode");
```

Also replace:
```rust
state.raft_node.list_roles()
```
with:
```rust
state.raft_node.as_ref().expect("list_roles requires cluster mode").list_roles()
```

And:
```rust
state.raft_node.get_role_by_name(&name)
```
with:
```rust
state.raft_node.as_ref().expect("get_role requires cluster mode").get_role_by_name(&name)
```

- [ ] **Step 3: Compile**

```bash
cargo check -p server-http 2>&1 | grep "^error" | head -20
```

Expected: errors only in `handlers/raft.rs`.

- [ ] **Step 4: Commit**

```bash
git add server-http/src/handlers/admin/users.rs server-http/src/handlers/admin/roles.rs
git commit -m "refactor(server-http/admin): use Option<RaftCacheNode> via expect in user/role handlers"
```

---

## Task 7: Update Raft metric handlers

**Files:**
- Modify: `server-http/src/handlers/raft.rs`

- [ ] **Step 1: Update `raft_metrics` and `cluster_nodes`**

In `server-http/src/handlers/raft.rs`, replace every `&state.raft_node` / `state.raft_node` access:

```rust
/// GET /raft/metrics
pub async fn raft_metrics(State(state): State<AppState>) -> Json<Value> {
    let node = state.raft_node.as_ref().expect("raft_metrics requires cluster mode");
    let m = node.raft.metrics().borrow().clone();
    Json(json!({
        "mode": "cluster",
        "id": m.id,
        "current_leader": m.current_leader,
        "current_term": m.current_term,
        "last_log_index": m.last_log_index,
        "last_applied": m.last_applied,
        "membership": m.membership_config.voter_ids().collect::<Vec<_>>(),
        "state": format!("{:?}", m.state),
    }))
}

/// GET /cluster/nodes
pub async fn cluster_nodes(State(state): State<AppState>) -> Json<Value> {
    let node = state.raft_node.as_ref().expect("cluster_nodes requires cluster mode");
    let leader_id = node.current_leader_id();
    let my_id = node.node_id();
    let all_nodes: Vec<_> = node.get_all_nodes().into_iter().collect();

    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(500))
        .build()
        .unwrap_or_default();

    let probes: Vec<_> = all_nodes
        .iter()
        .map(|(id, n)| {
            let client = client.clone();
            let probe_addr = n.http_addr.replace("0.0.0.0", "127.0.0.1");
            let url = format!("http://{}/health", probe_addr);
            let id = *id;
            async move {
                let reachable = timeout(Duration::from_millis(500), client.get(&url).send())
                    .await
                    .map(|r| r.is_ok())
                    .unwrap_or(false);
                (id, reachable)
            }
        })
        .collect();

    let results: Vec<(u64, bool)> = futures::future::join_all(probes).await;

    let nodes: Vec<Value> = all_nodes
        .into_iter()
        .map(|(id, n)| {
            let reachable = results
                .iter()
                .find(|(rid, _)| *rid == id)
                .map(|(_, r)| *r)
                .unwrap_or(false);
            json!({
                "id": id,
                "raft_addr": n.raft_addr,
                "http_addr": n.http_addr,
                "tcp_addr": n.tcp_addr,
                "is_leader": leader_id == Some(id),
                "is_self": id == my_id,
                "reachable": reachable,
            })
        })
        .collect();

    Json(json!({ "mode": "cluster", "nodes": nodes }))
}
```

- [ ] **Step 2: Full compile of server-http**

```bash
cargo check -p server-http 2>&1
```

Expected: clean.

- [ ] **Step 3: Commit**

```bash
git add server-http/src/handlers/raft.rs
git commit -m "refactor(server-http/raft): use Option<RaftCacheNode> via expect in raft handlers"
```

---

## Task 8: Create `carbon-server/src/standalone.rs`

This is the pre-Raft bootstrap path. Auth is persisted directly to redb files; cache data is pure in-memory `CacheManager`.

**Files:**
- Create: `carbon-server/src/standalone.rs`

- [ ] **Step 1: Create `carbon-server/src/standalone.rs`**

```rust
use carbon::auth::{
    AuthService, MokaSessionRepository, RedbRoleRepository, RedbUserRepository, RoleRepository,
    RoleService, SessionStore, UserService,
    defaults::{create_default_admin, create_default_roles},
};
use carbon::planes::control::CacheManager;
use carbon::planes::data::cache_operations::CacheOperationsService;
use carbon::planes::data::operation::CacheOperations;
use bytes::Bytes;
use shared::config::Config;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tracing::{info, warn};

pub async fn start(config: Arc<Config>) -> Result<(), Box<dyn std::error::Error>> {
    info!("Mode: standalone (no Raft consensus)");

    // ── Cache ──────────────────────────────────────────────────────────────
    let cache_manager = CacheManager::<Vec<u8>, Bytes>::new();
    let cache_ops: Arc<dyn CacheOperations<Vec<u8>, Bytes>> =
        Arc::new(CacheOperationsService::new(cache_manager));

    // ── Auth ───────────────────────────────────────────────────────────────
    let auth_base = std::path::Path::new(&config.data_dir).join(".carbon");
    std::fs::create_dir_all(&auth_base)?;

    let user_repo = Arc::new(
        RedbUserRepository::new(auth_base.join("users.redb"))
            .expect("Failed to open users.redb"),
    );
    let role_repo = Arc::new(
        RedbRoleRepository::new(auth_base.join("roles.redb"))
            .expect("Failed to open roles.redb"),
    ) as Arc<dyn carbon::auth::RoleRepository>;

    let auth_service = Arc::new(AuthService::new(user_repo.clone(), role_repo.clone()));
    let user_service = Arc::new(UserService::new(user_repo.clone(), role_repo.clone()));
    let role_service = Arc::new(RoleService::new(role_repo.clone()));

    init_auth_defaults(&user_repo, &role_repo, &config.admin_username, &config.admin_password).await;

    let session_repository = Arc::new(MokaSessionRepository::new(
        None,
        Some(Duration::from_secs(3600)),
    ));
    let session_store = Arc::new(SessionStore::new(session_repository));

    // ── HTTP ───────────────────────────────────────────────────────────────
    let app_state = server_http::AppState::new(
        cache_ops.clone(),
        None, // no raft node in standalone mode
        auth_service,
        user_service,
        role_service,
        session_store,
    );

    let http_router = server_http::build_router(app_state, &config);

    // ── TCP ────────────────────────────────────────────────────────────────
    let config_tcp = Arc::clone(&config);
    let tcp_ops = cache_ops.clone();

    let tcp_handle = tokio::spawn(async move {
        info!("Starting TCP server on {}:{}", config_tcp.host, config_tcp.tcp.port());
        let listener =
            TcpListener::bind(format!("{}:{}", config_tcp.host, config_tcp.tcp.port()))
                .await
                .expect("Failed to bind TCP server");
        info!("TCP server listening on {}:{}", config_tcp.host, config_tcp.tcp.port());
        loop {
            match listener.accept().await {
                Ok((socket, addr)) => {
                    let ops = tcp_ops.clone();
                    tokio::spawn(async move {
                        if let Err(e) = server_tcp::process_connection(socket, ops).await {
                            tracing::warn!("TCP {addr} error: {e:?}");
                        }
                    });
                }
                Err(e) => tracing::error!("TCP accept error: {e}"),
            }
        }
    });

    let config_http = Arc::clone(&config);
    let http_handle = tokio::spawn(async move {
        info!("Starting HTTP server on {}:{}", config_http.host, config_http.http.port());
        let listener =
            TcpListener::bind(format!("{}:{}", config_http.host, config_http.http.port()))
                .await
                .expect("Failed to bind HTTP server");
        info!("HTTP server listening on {}:{}", config_http.host, config_http.http.port());
        axum::serve(listener, http_router)
            .with_graceful_shutdown(shutdown_signal())
            .await
            .expect("HTTP server error");
    });

    info!("Carbon standalone server started");
    info!("  HTTP : {}://{}:{}", config.http.http_protcol(), config.host, config.http.port());
    info!("  TCP  : {}:{}", config.host, config.tcp.port());

    tokio::select! {
        _ = http_handle => info!("HTTP server task completed"),
        _ = tcp_handle  => info!("TCP server task completed"),
        _ = shutdown_signal() => info!("Shutdown signal received"),
    }

    info!("Carbon standalone server shutting down");
    Ok(())
}

async fn init_auth_defaults(
    user_repo: &Arc<RedbUserRepository>,
    role_repo: &Arc<dyn carbon::auth::RoleRepository>,
    admin_username: &str,
    admin_password: &str,
) {
    let role_service = RoleService::new(role_repo.clone());
    let default_roles = match role_service.initialize_default_roles().await {
        Ok(r) => r,
        Err(e) => {
            warn!("Failed to initialize default roles: {e}");
            return;
        }
    };

    let admin_role_id = default_roles
        .iter()
        .find(|r| r.name == "admin")
        .map(|r| r.id.clone())
        .unwrap_or_default();

    let exists = user_repo.username_exists(admin_username).await.unwrap_or(false);
    if !exists {
        match create_default_admin(admin_username.to_string(), admin_password.to_string(), admin_role_id) {
            Ok(user) => {
                if let Err(e) = user_repo.create(user).await {
                    warn!("Failed to create default admin: {e}");
                } else {
                    info!("Created default admin user: {admin_username}");
                }
            }
            Err(e) => warn!("Failed to hash admin password: {e}"),
        }
    } else {
        info!("Admin user already exists: {admin_username}");
    }
}

async fn shutdown_signal() {
    use tokio::signal;
    let ctrl_c = async { signal::ctrl_c().await.expect("Failed to install Ctrl+C handler") };
    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install signal handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => info!("Received Ctrl+C"),
        _ = terminate => info!("Received terminate"),
    }
}
```

- [ ] **Step 2: Add `mod standalone;` to `carbon-server/src/main.rs`** (just add the mod declaration for now — we'll rewrite main in Task 10):

```rust
mod standalone;
```

Add it at the top of `main.rs`, before `use` statements.

- [ ] **Step 3: Compile**

```bash
cargo check -p carbon-server 2>&1 | grep "^error" | head -30
```

- [ ] **Step 4: Commit**

```bash
git add carbon-server/src/standalone.rs carbon-server/src/main.rs
git commit -m "feat(carbon-server): add standalone bootstrap module"
```

---

## Task 9: Create `carbon-server/src/cluster.rs`

Extract the current `main.rs` Raft bootstrap into a `pub async fn start(config: Arc<Config>)` function.

**Files:**
- Create: `carbon-server/src/cluster.rs`

- [ ] **Step 1: Create `carbon-server/src/cluster.rs`**

```rust
use carbon::auth::{
    AuthService, MokaSessionRepository, RoleRepository, RoleService, SessionStore, UserRepository,
    UserService,
    defaults::{create_default_admin, create_default_roles},
};
use carbon::planes::data::operation::CacheOperations;
use carbon_raft::log_store::CarbonRaftStorage;
use carbon_raft::node::RaftCacheNode;
use carbon_raft::types::RaftLogEntry;
use carbon_raft::{RaftRoleRepository, RaftUserRepository};
use bytes::Bytes;
use shared::config::Config;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tracing::{info, warn};

pub async fn start(config: Arc<Config>) -> Result<(), Box<dyn std::error::Error>> {
    info!(
        "Mode: cluster (Raft consensus) — node_id={}, raft_addr={}, seeds={:?}",
        config.node_id, config.raft_addr, config.seeds
    );

    // ── Raft node ──────────────────────────────────────────────────────────
    std::fs::create_dir_all(&config.cluster_data_dir)?;
    let db_path = std::path::Path::new(&config.cluster_data_dir).join("raft.redb");
    let storage = CarbonRaftStorage::open(db_path)?;
    let raft_node = RaftCacheNode::new(
        config.node_id,
        config.raft_addr.clone(),
        config.http_addr.clone(),
        config.tcp_addr.clone(),
        config.seeds.clone(),
        storage,
    )
    .await?;

    info!("Raft node started");

    // ── Auth (Raft-backed) ─────────────────────────────────────────────────
    let session_repository = Arc::new(MokaSessionRepository::new(
        None,
        Some(Duration::from_secs(3600)),
    ));
    let session_store = Arc::new(SessionStore::new(session_repository));

    let user_repo = Arc::new(RaftUserRepository { sm: raft_node.state.clone() }) as Arc<dyn UserRepository>;
    let role_repo = Arc::new(RaftRoleRepository { sm: raft_node.state.clone() }) as Arc<dyn RoleRepository>;

    let auth_service = Arc::new(AuthService::new(user_repo.clone(), role_repo.clone()));
    let user_service = Arc::new(UserService::new(user_repo.clone(), role_repo.clone()));
    let role_service = Arc::new(RoleService::new(role_repo));

    let node_for_defaults = raft_node.clone();
    let admin_username = config.admin_username.clone();
    let admin_password = config.admin_password.clone();
    tokio::spawn(async move {
        init_raft_auth_defaults(node_for_defaults, admin_username, admin_password).await;
    });

    // ── HTTP ───────────────────────────────────────────────────────────────
    let cache_ops: Arc<dyn CacheOperations<Vec<u8>, Bytes>> = raft_node.clone();
    let app_state = server_http::AppState::new(
        cache_ops,
        Some(raft_node.clone()),
        auth_service,
        user_service,
        role_service,
        session_store,
    );

    let http_router = server_http::build_router(app_state, &config);

    // ── TCP ────────────────────────────────────────────────────────────────
    let config_tcp = Arc::clone(&config);
    let tcp_node = raft_node.clone();

    let tcp_handle = tokio::spawn(async move {
        info!("Starting TCP server on {}:{}", config_tcp.host, config_tcp.tcp.port());
        let listener =
            TcpListener::bind(format!("{}:{}", config_tcp.host, config_tcp.tcp.port()))
                .await
                .expect("Failed to bind TCP server");
        info!("TCP server listening on {}:{}", config_tcp.host, config_tcp.tcp.port());
        loop {
            match listener.accept().await {
                Ok((socket, addr)) => {
                    let ops: Arc<dyn CacheOperations<Vec<u8>, Bytes>> = tcp_node.clone();
                    tokio::spawn(async move {
                        if let Err(e) = server_tcp::process_connection(socket, ops).await {
                            tracing::warn!("TCP {addr} error: {e:?}");
                        }
                    });
                }
                Err(e) => tracing::error!("TCP accept error: {e}"),
            }
        }
    });

    let config_http = Arc::clone(&config);
    let http_handle = tokio::spawn(async move {
        info!("Starting HTTP server on {}:{}", config_http.host, config_http.http.port());
        let listener =
            TcpListener::bind(format!("{}:{}", config_http.host, config_http.http.port()))
                .await
                .expect("Failed to bind HTTP server");
        info!(
            "HTTP server listening on {}://{}:{}",
            config_http.http.http_protcol(), config_http.host, config_http.http.port()
        );
        axum::serve(listener, http_router)
            .with_graceful_shutdown(shutdown_signal())
            .await
            .expect("HTTP server error");
    });

    info!("Carbon cluster server started");
    info!("  HTTP : {}://{}:{}", config.http.http_protcol(), config.host, config.http.port());
    info!("  TCP  : {}:{}", config.host, config.tcp.port());
    info!("  Raft RPC : {}", config.raft_addr);

    tokio::select! {
        _ = http_handle => info!("HTTP server task completed"),
        _ = tcp_handle  => info!("TCP server task completed"),
        _ = shutdown_signal() => info!("Shutdown signal received"),
    }

    info!("Carbon cluster server shutting down");
    Ok(())
}

async fn init_raft_auth_defaults(
    node: Arc<RaftCacheNode>,
    admin_username: String,
    admin_password: String,
) {
    for _ in 0..40 {
        tokio::time::sleep(Duration::from_millis(250)).await;
        if node.raft.metrics().borrow().current_leader.is_some() {
            break;
        }
    }

    let metrics = node.raft.metrics().borrow().clone();
    let Some(leader_id) = metrics.current_leader else {
        warn!("No Raft leader elected — skipping auth default bootstrap");
        return;
    };
    if leader_id != metrics.id {
        info!("Not the leader — skipping auth defaults (leader is node {leader_id})");
        return;
    }

    info!("Leader elected — bootstrapping default roles and admin user via Raft");

    let default_roles = create_default_roles();
    let mut admin_role_id = String::new();

    for role in default_roles {
        if role.name == "admin" {
            admin_role_id = role.id.clone();
        }
        if node.get_role_by_name(&role.name).await.is_none() {
            match node.write(RaftLogEntry::CreateRole(role.clone())).await {
                Ok(_) => info!("Created default role: {}", role.name),
                Err(e) => warn!("Failed to create default role {}: {e}", role.name),
            }
        }
    }

    if admin_role_id.is_empty() {
        if let Some(r) = node.get_role_by_name("admin").await {
            admin_role_id = r.id;
        }
    }

    if !node.username_exists(&admin_username).await {
        match create_default_admin(admin_username.clone(), admin_password, admin_role_id) {
            Ok(user) => match node.write(RaftLogEntry::CreateUser(user)).await {
                Ok(_) => info!("Created default admin user: {admin_username}"),
                Err(e) => warn!("Failed to write admin user to Raft: {e}"),
            },
            Err(e) => warn!("Failed to hash admin password: {e}"),
        }
    } else {
        info!("Admin user already exists in cluster state");
    }
}

async fn shutdown_signal() {
    use tokio::signal;
    let ctrl_c = async { signal::ctrl_c().await.expect("Failed to install Ctrl+C handler") };
    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install signal handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => info!("Received Ctrl+C"),
        _ = terminate => info!("Received terminate"),
    }
}
```

- [ ] **Step 2: Add `mod cluster;` to `carbon-server/src/main.rs`**

Add at the top of `main.rs`:
```rust
mod cluster;
```

- [ ] **Step 3: Compile**

```bash
cargo check -p carbon-server 2>&1 | grep "^error" | head -30
```

- [ ] **Step 4: Commit**

```bash
git add carbon-server/src/cluster.rs carbon-server/src/main.rs
git commit -m "feat(carbon-server): add cluster bootstrap module"
```

---

## Task 10: Rewrite `carbon-server/src/main.rs`

**Files:**
- Modify: `carbon-server/src/main.rs`

- [ ] **Step 1: Replace `main.rs` with the thin dispatcher**

```rust
mod cluster;
mod standalone;

use shared::config::{Config, ServerMode};
use std::sync::Arc;
use tracing::{Level, info};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .with_ansi(std::io::IsTerminal::is_terminal(&std::io::stderr()))
        .init();

    info!("Starting Carbon Server");

    match dotenvy::dotenv() {
        Ok(_) => info!("Loaded environment variables from .env file"),
        Err(_) => info!("No .env file found, using system environment variables"),
    }

    let config = Arc::new(Config::from_env());

    match config.mode {
        ServerMode::Standalone => standalone::start(config).await,
        ServerMode::Cluster => cluster::start(config).await,
    }
}
```

- [ ] **Step 2: Full workspace compile**

```bash
cargo build -p carbon-server 2>&1
```

Expected: clean build.

- [ ] **Step 3: Commit**

```bash
git add carbon-server/src/main.rs
git commit -m "refactor(carbon-server): rewrite main.rs to dispatch to standalone or cluster module"
```

---

## Task 11: Verify standalone mode works end-to-end

- [ ] **Step 1: Start in standalone mode**

```bash
cd /Users/chirdeeptomar/opensource/carbon
CARBON_MODE=standalone cargo run -p carbon-server 2>&1 &
sleep 2
```

Expected log lines:
```
Mode: standalone (no Raft consensus)
Carbon standalone server started
  HTTP : http://localhost:8080
  TCP  : localhost:5500
```

- [ ] **Step 2: Health check**

```bash
curl -s http://localhost:8080/health
```

Expected: `{"status":"ok"}` or similar 200 response.

- [ ] **Step 3: Create a cache and write a value**

```bash
# Login
TOKEN=$(curl -s -X POST http://localhost:8080/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"username":"admin","password":"admin123"}' | jq -r '.token')

# Create cache
curl -s -X POST http://localhost:8080/admin/caches \
  -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"name":"test","eviction":"ttl","ttl_ms":60000,"max_size":1000}'

# Write value
curl -s -X PUT http://localhost:8080/cache/test/hello \
  -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"value":"world"}'

# Read value
curl -s http://localhost:8080/cache/test/hello \
  -H "Authorization: Bearer $TOKEN"
```

Expected final response: `{"found":true,"value":"world","ttl_ms_remaining":0}`

- [ ] **Step 4: Stop standalone server**

```bash
kill %1 2>/dev/null || pkill -f "carbon-server" 2>/dev/null
```

- [ ] **Step 5: Commit verification note**

```bash
git commit --allow-empty -m "chore: verified standalone mode boots and serves cache ops correctly"
```

---

## Task 12: Verify cluster mode works end-to-end (two nodes)

- [ ] **Step 1: Start node 1 (bootstrap)**

```bash
cd /Users/chirdeeptomar/opensource/carbon
CARBON_MODE=cluster \
CARBON_NODE_ID=1 \
CARBON_HTTP_PORT=8080 \
CARBON_TCP_PORT=5500 \
CARBON_RAFT_PORT=8091 \
CARBON_CLUSTER_DATA_DIR=./data/raft/1 \
cargo run -p carbon-server 2>&1 | tee /tmp/node1.log &
sleep 3
```

Expected: `Leader elected — bootstrapping default roles and admin user via Raft`

- [ ] **Step 2: Start node 2 (joins node 1)**

```bash
CARBON_MODE=cluster \
CARBON_NODE_ID=2 \
CARBON_HTTP_PORT=8081 \
CARBON_TCP_PORT=5501 \
CARBON_RAFT_PORT=8092 \
CARBON_CLUSTER_DATA_DIR=./data/raft/2 \
CARBON_SEEDS=0.0.0.0:8091 \
cargo run -p carbon-server 2>&1 | tee /tmp/node2.log &
sleep 3
```

Expected in node 2 logs: joined cluster, node 1 is leader.

- [ ] **Step 3: Check cluster membership**

```bash
curl -s http://localhost:8080/cluster/nodes | jq .
```

Expected: two nodes listed, one marked `is_leader: true`.

- [ ] **Step 4: Write on node 1, read on node 2**

```bash
TOKEN=$(curl -s -X POST http://localhost:8080/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"username":"admin","password":"admin123"}' | jq -r '.token')

curl -s -X POST http://localhost:8080/admin/caches \
  -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"name":"cluster-test","eviction":"ttl","ttl_ms":60000,"max_size":1000}'

curl -s -X PUT http://localhost:8080/cache/cluster-test/key1 \
  -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"value":"replicated"}'

# Read from node 2 (follower)
curl -s http://localhost:8081/cache/cluster-test/key1 \
  -H "Authorization: Bearer $TOKEN"
```

Expected: `{"found":true,"value":"replicated","ttl_ms_remaining":0}`

- [ ] **Step 5: Tear down**

```bash
pkill -f "carbon-server" 2>/dev/null
rm -rf ./data/raft
```

- [ ] **Step 6: Final commit**

```bash
git commit --allow-empty -m "chore: verified cluster mode boots two-node cluster and replicates writes"
```

---

## Self-Review Checklist

**Spec coverage:**
- ✅ `ServerMode` enum + `CARBON_MODE` env var — Task 1
- ✅ `standalone.rs` module with pre-Raft bootstrap — Task 8
- ✅ `cluster.rs` module with Raft bootstrap — Task 9
- ✅ `main.rs` dispatches to one module — Task 10
- ✅ `server-http` AppState generalised — Tasks 3–7
- ✅ `server-tcp` generalised — Task 2
- ✅ Both modes verified end-to-end — Tasks 11–12

**Placeholder scan:** None found — all steps include complete code or exact commands.

**Type consistency:**
- `AppState::new` takes `Arc<dyn CacheOperations<Vec<u8>, Bytes>>` (Tasks 3, 8, 9) ✅
- `process_connection` takes `Arc<dyn CacheOperations<Vec<u8>, Bytes>>` (Tasks 2, 8, 9) ✅
- `state.raft_node` is `Option<Arc<RaftCacheNode>>` everywhere (Tasks 3–7) ✅
- `ServerMode` imported as `shared::config::ServerMode` in main.rs (Tasks 1, 10) ✅
