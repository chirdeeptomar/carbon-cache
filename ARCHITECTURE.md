# Carbon Cache — Architecture

Carbon Cache is a Rust cache server exposing both an HTTP API and a raw TCP
protocol over a shared cache/auth core. It runs in one of two modes chosen at
startup via `CARBON_MODE`:

- **Standalone** — single node, no consensus, redb-backed persistence.
- **Cluster** — multiple nodes replicated via Raft (`openraft`), writes go
  through consensus, reads are served locally.

Both modes assemble the *same* HTTP router and TCP handler; they differ only
in which implementation of the core traits (`CacheOperations`,
`AdminOperations`, `UserRepository`, `RoleRepository`) gets wired in at boot.
That trait boundary is the central architectural idea in this codebase.

---

## 1. Workspace layout

```text
    +--------------------------------------+
    |  Entrypoint: carbon-server           |
    |  main.rs, standalone.rs, cluster.rs  |
    +--------------------------------------+

    depends on all of:

    +-----------------------------------------+   +-----------------------+
    |  server-http                            |   |  server-tcp           |
    |  Axum HTTP API + admin UI static serve  |   |  binary TCP protocol  |
    +-----------------------------------------+   +-----------------------+

    +----------------------------------+   +--------------------------------+
    |  carbon-raft                     |   |  shared                        |
    |  openraft node, log store, RPC,  |   |  Config, ServerMode, Protocol  |
    |  auth-over-Raft                  |   |                                |
    +----------------------------------+   +--------------------------------+

    server-http and server-tcp both depend on:

    +---------------------------------------------+
    |  carbon                                     |
    |  domain model, auth, control & data planes  |
    +---------------------------------------------+

    carbon-raft also depends on carbon (same box as above).

    carbon depends on:

    +-------------------------------+
    |  storage-engine               |
    |  Moka / Foyer cache backends  |
    +-------------------------------+

    server-http additionally depends on:

    +------------------------------+
    |  shared-http                 |
    |  HTTP request/response DTOs  |
    +------------------------------+

    Separately, an external client:

    +--------------------------------+
    |  clients/carbon-client-python  |
    |  HTTP + TCP client library     |
    +--------------------------------+
      -> HTTP admin ops go to server-http
      -> TCP get/put/delete go to server-tcp
```

Crate responsibilities:

| Crate | Responsibility |
| --- | --- |
| `carbon-server` | Binary entrypoint. Reads `Config`, dispatches to `standalone::start` or `cluster::start`. |
| `carbon` | Domain types (`CacheConfig`, `CacheInfo`), auth (users/roles/sessions/passwords), control plane (`CacheManager`, `AdminOperations`) and data plane (`CacheOperationsService`, `CacheOperations`) traits and default implementations. |
| `carbon-raft` | `openraft`-based Raft node (`RaftCacheNode`), redb-backed log store, network/RPC layer, and Raft-backed `UserRepository`/`RoleRepository` implementations. |
| `server-http` | Axum router, HTTP handlers (cache, admin, auth, raft metrics), auth/authz middleware, leader-forwarding for cluster mode. |
| `server-tcp` | Length-delimited binary protocol server (`process_connection`), leader-forwarding equivalent for TCP. |
| `storage-engine` | `UnifiedStorageFactory` — builds the actual in-memory/disk cache instance (Moka or Foyer) per `CacheConfig`. |
| `shared` / `shared-http` | Cross-crate config (`ServerMode`, `Protocol`, env parsing) and HTTP DTOs shared between server and client. |
| `clients/carbon-client-python` | Python client: HTTP transport for admin ops, TCP transport (with connection pooling / cluster failover) for hot-path get/put/delete. |

---

## 2. The trait boundary: standalone vs cluster

This is the architectural crux. `server-http` and `server-tcp` are written
entirely against trait objects — they never know whether they're talking to
a local redb store or a Raft-replicated state machine.

```text
    +--------------------------------------------------------------------+
    |  Core traits (carbon crate)                                        |
    |                                                                    |
    |  CacheOperations<K,V>   - get / put / delete                       |
    |  AdminOperations<K,V>   - create_cache / drop_cache / list_caches  |
    |  UserRepository                                                    |
    |  RoleRepository                                                    |
    +--------------------------------------------------------------------+

    Each trait has exactly two implementations:

    +-------------------------------+    +-------------------------------------+
    |  Standalone implementations   |    |  Cluster implementations            |
    |                               |    |                                     |
    |  CacheOperationsService<K,V>  |    |  RaftCacheNode (carbon-raft)        |
    |    (carbon::planes::data)     |    |    implements BOTH CacheOperations  |
    |  CacheManager<K,V>            |    |    AND AdminOperations directly     |
    |    (carbon::planes::control)  |    |  RaftUserRepository                 |
    |  RedbUserRepository           |    |  RaftRoleRepository                 |
    |  RedbRoleRepository           |    |                                     |
    +-------------------------------+    +-------------------------------------+

    Both sets are wired into the same consumer:

    +--------------------------------------+
    |  server_http::AppState               |
    |  server_tcp::process_connection      |
    |                                      |
    |  (depends only on the traits above,  |
    |   never on which implementation      |
    |   is behind them)                    |
    +--------------------------------------+
```

- **Standalone** (`carbon-server/src/standalone.rs`): `CacheManager` +
  `CacheOperationsService` wrap `UnifiedStorageFactory` (Moka/Foyer) with
  cache *config* persisted to a local redb file. Users/roles persist to
  `RedbUserRepository` / `RedbRoleRepository`, also redb-backed.
- **Cluster** (`carbon-server/src/cluster.rs`): `RaftCacheNode` implements
  both `CacheOperations` and `AdminOperations` directly — writes go through
  `openraft` consensus and get applied to `CacheStateMachine`. Users/roles
  are read/written via `RaftUserRepository`/`RaftRoleRepository`, which
  route through the same Raft log (`sm: raft_node.state.clone()`).

Sessions (`MokaSessionRepository`) are **not** replicated in either mode —
each node keeps its own in-memory session cache with a TTL.

---

## 3. Request flow (standalone)

```text
    Actors:
    +-----------------------------------------------+
    |  Client        - caller                       |
    |  HTTP          - server-http (Axum)           |
    |  Auth          - AuthService / middleware     |
    |  CacheOps      - CacheOperationsService       |
    |  Storage       - storage-engine (Moka/Foyer)  |
    |  Redb          - redb (auth + cache config)   |
    +-----------------------------------------------+

    Steps:

     1. Client  -> HTTP     : PUT /cache/{name}/{key}
     2. HTTP    -> Auth     : authenticate() + authorize()
     3. Auth    -> Redb     : look up session / user / role
     4. Redb    -> Auth     : (session/user/role data)
     5. Auth    -> HTTP     : OK
     6. HTTP    -> CacheOps : put(key, value)
     7. CacheOps-> Storage  : write to backend cache
     8. Storage -> CacheOps : ok
     9. CacheOps-> HTTP     : PutResponse
    10. HTTP    -> Client   : 200 OK
```

## 4. Request flow (cluster, write path)

```text
    Actors:
    +--------------------------------------------------+
    |  Client      - caller                            |
    |  HTTP        - server-http (follower or leader)  |
    |  Proxy       - leader_proxy::forward_to_leader   |
    |  Raft        - RaftCacheNode (leader)            |
    |  Consensus   - openraft consensus                |
    |  SM          - CacheStateMachine                 |
    +--------------------------------------------------+

    Steps:

     1. Client -> HTTP      : PUT /cache/{name}/{key}
     2. HTTP   -> HTTP      : check current_leader_id()

          if this node is NOT the leader:
     3.     HTTP  -> Proxy  : forward_to_leader(request)
     4.     Proxy -> Raft   : HTTP call to leader's address
          else (this node IS the leader):
     5.     HTTP  -> Raft   : write(RaftLogEntry)

     6. Raft   -> Consensus : replicate log entry to quorum
     7. Consensus -> Raft   : committed
     8. Raft   -> SM        : apply_to_state_machine()
     9. SM     -> Raft      : RaftResponse
    10. Raft   -> Client    : PutResponse
```

The same forwarding pattern exists independently in `server-tcp`
(`leader_proxy::forward_to_leader`) — the graph flagged this as a
near-duplicate of the HTTP version, since both protocol servers implement
leader forwarding separately rather than sharing one implementation.

---

## 5. Storage backend selection

```text
    +------------------------------------+
    |  CacheConfig                       |
    |  (backend: CacheEvictionStrategy)  |
    +------------------------------------+

    -> UnifiedStorageFactory::create_from_config() picks one of:

    +-------------------------+   +---------------------------+
    |  MokaCache<K,V>         |   |  FoyerMemoryCache<K,V>    |
    |  TTL-based eviction     |   |  size-bounded, in-memory  |
    |                         |   |                           |
    |  (backend = TimeBound)  |   |  (backend = SizeBounded)  |
    +-------------------------+   +---------------------------+

    +----------------------------------+
    |  Foyer hybrid                    |
    |  disk overflow                   |
    |                                  |
    |  (backend = SizeBounded + disk)  |
    +----------------------------------+
```

Each cache created via `POST /admin/caches` picks its backend at creation
time based on the requested eviction strategy (`lru`/TTL → Moka,
size-bounded → Foyer, with optional disk overflow).

---

## 6. Auth & session model

```text
    +--------------------+
    |  POST /auth/login  |
    +--------------------+

    -> AuthService::authenticate() calls all of:

    +----------------------+
    |  password.rs         |
    |  argon2 hash/verify  |
    +----------------------+

    +--------------------------+   +--------------------------+
    |  UserRepository          |   |  RoleRepository          |
    |  (Redb- or Raft-backed)  |   |  (Redb- or Raft-backed)  |
    +--------------------------+   +--------------------------+

    -> and populates:

    +----------------+
    |  SessionStore  |
    +----------------+

    -> backed by:

    +----------------------------+
    |  MokaSessionRepository     |
    |  in-memory, per-node, TTL  |
    +----------------------------+

    Separately, on every protected request:

    +---------------------------------------------------+
    |  authentication.rs / authorization.rs middleware  |
    |                                                   |
    |  -> uses SessionStore (validate session)          |
    |  -> uses RoleRepository (check_permission)        |
    +---------------------------------------------------+
```

Permissions are role-based (`Role` → list of `Permission`); every protected
handler calls `check_permission`/`check_any_permission` from the
authorization middleware rather than relying on router-level gating alone.

---

## 7. Notes from the dependency graph

Generated via [graphify](graphify-out/GRAPH_REPORT.md) (103 communities,
1453 nodes over the current source tree). Points worth flagging:

- **Duplicated PUT/DELETE logic**: `server-http`'s `put_value`/`delete_value`
  handlers and `server-tcp`'s `do_put`/`do_delete` helpers implement the same
  operation independently against the same `CacheOperations` trait — no
  shared handler code between the two protocol servers.
- **Parallel leader-forwarding**: `server-http::leader_proxy` and
  `server-tcp::leader_proxy` are separate implementations of the same
  "forward write to Raft leader" behavior.
- **Symmetric repository pairs**: `RaftUserRepository`/`RedbUserRepository`
  and `RaftRoleRepository`/`RedbRoleRepository` are structurally parallel —
  same trait, same method set, different storage substrate.
- **Migration history visible in the graph**: `docs/superpowers/specs/` and
  `docs/superpowers/plans/` capture two prior migrations that shaped this
  structure — sled → redb (persistence) and the standalone/cluster mode
  split — plus an in-flight Axum → Actix-Web migration design for
  `server-http`.
