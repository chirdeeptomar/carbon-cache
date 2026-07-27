# Axum → Actix-Web Migration Design

**Date:** 2026-06-13  
**Status:** Approved for implementation  
**Scope:** `server-http` crate only — no changes to TCP server, Raft layer, or domain logic

---

## Context

Carbon uses Axum as its HTTP framework. Actix-Web consistently outperforms Axum on TechEmpower and similar benchmarks due to its actor-model concurrency and lower per-request overhead. For an open source cache server, benchmark numbers meaningfully influence adoption. The migration replaces the HTTP framework while leaving everything below the HTTP layer untouched — AppState, AuthService, RaftCacheNode, TCP server, protocol, and all domain logic are unchanged.

---

## What Changes

Only `server-http` and the two server bootstrap files that call `axum::serve`:

| File | Change |
|---|---|
| `server-http/Cargo.toml` | Swap `axum`, `tower-http` for `actix-web`, `actix-cors`, `actix-files` |
| `server-http/src/routes.rs` | Rewrite router as `actix_web::App` with `.service()` and `.wrap()` |
| `server-http/src/middleware/authentication.rs` | Rewrite as Actix `from_fn` middleware |
| `server-http/src/middleware/authorization.rs` | Update types (still inline checks in handlers, not a layer) |
| `server-http/src/handlers/cache/basic.rs` | Update extractors: `Path`, `Json`, `web::Data` |
| `server-http/src/handlers/cache/health.rs` | Trivial — update response type |
| `server-http/src/handlers/cache/events.rs` | Rewrite SSE using `actix-web`'s streaming body |
| `server-http/src/handlers/auth.rs` | Update extractors and response types |
| `server-http/src/handlers/admin/cache.rs` | Update extractors and body handling |
| `server-http/src/handlers/admin/users.rs` | Update `Extension<User>` → `req.extensions()` |
| `server-http/src/handlers/admin/roles.rs` | Update `Extension<User>` → `req.extensions()` |
| `server-http/src/handlers/raft.rs` | Minimal — update response type |
| `server-http/src/leader_proxy.rs` | Update `Request`/`Response` body types |
| `carbon-server/src/standalone.rs` | Replace `axum::serve` with `HttpServer::new().bind().run()` |
| `carbon-server/src/cluster.rs` | Same as standalone |

**What does NOT change:** `AppState`, `AuthService`, `RaftCacheNode`, `CacheOperations`, all domain types, TCP server, Raft layer, `shared` crate, `carbon` crate.

---

## Architecture

### App construction

Axum builds a `Router` and passes it to `axum::serve`. Actix builds a closure-based `HttpServer` factory:

```rust
// Before (Axum)
let router = build_router(state, &config);
axum::serve(listener, router).with_graceful_shutdown(signal).await?;

// After (Actix-Web)
HttpServer::new(move || {
    build_app(state.clone(), &config)
})
.bind((host, port))?
.run()
.await?;
```

`build_app` returns an `actix_web::App` with middleware and routes configured.

### State sharing

Axum uses `State<AppState>` extractor. Actix uses `web::Data<AppState>`:

```rust
// Before
async fn handler(State(state): State<AppState>) { ... }

// After
async fn handler(state: web::Data<AppState>) { ... }
```

`AppState` is wrapped in `web::Data::new(state)` and attached via `.app_data()`. All `Arc` fields inside `AppState` remain unchanged.

### Middleware

Axum middleware is an async function `(State<S>, Request, Next) -> Response`. Actix-Web `from_fn` middleware is similar but uses `actix_web::middleware::from_fn`:

```rust
// Before (Axum)
async fn auth_middleware(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Response { ... }

// After (Actix-Web)
async fn auth_middleware(
    req: ServiceRequest,
    next: Next<impl MessageBody>,
) -> Result<ServiceResponse<impl MessageBody>, Error> { ... }
```

State is accessed from `req.app_data::<web::Data<AppState>>()` inside the middleware rather than injected as a parameter. The logic (Bearer/Basic auth, session validation, IP extraction, User injection into extensions) is identical — only the wrapping types change.

### User injection via extensions

Both frameworks support request extensions. The pattern stays the same:

```rust
// Insert in middleware (Actix)
req.extensions_mut().insert(user);

// Extract in handler (Actix)
let user = req.extensions().get::<User>().cloned();
```

### Route registration

```rust
// Before (Axum)
Router::new()
    .route("/cache/{cache_name}/{key}", get(get_value).put(put_value).delete(delete_value))
    .layer(middleware::from_fn_with_state(state.clone(), auth_middleware))

// After (Actix-Web)
web::scope("")
    .wrap(from_fn(auth_middleware))
    .service(web::resource("/cache/{cache_name}/{key}")
        .route(web::get().to(get_value))
        .route(web::put().to(put_value))
        .route(web::delete().to(delete_value)))
```

### SSE (Server-Sent Events)

Axum's `Sse<impl Stream>` becomes an Actix streaming `HttpResponse`:

```rust
// After (Actix-Web)
async fn events(state: web::Data<AppState>) -> impl Responder {
    let rx = state.event_channel.subscribe();
    let stream = tokio_stream::wrappers::BroadcastStream::new(rx)
        .map(|event| { ... });
    HttpResponse::Ok()
        .content_type("text/event-stream")
        .streaming(stream)
}
```

### Graceful shutdown

Axum's `.with_graceful_shutdown()` is replaced by Actix's built-in signal handling. `HttpServer::run()` returns a `Server` handle; calling `.stop(true)` triggers graceful shutdown. Signal handling moves to the bootstrap files.

### CORS

`tower-http`'s `CorsLayer` is replaced by `actix-cors::Cors`:

```rust
App::new()
    .wrap(
        Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allowed_headers(vec![header::AUTHORIZATION, header::CONTENT_TYPE])
    )
```

---

## Key Invariants to Preserve

1. **Auth middleware must run before all protected routes** — same as today; only public routes (`/health`, `/raft/metrics`, `/cluster/nodes`) bypass it.
2. **`User` must be available in handler extensions** after auth middleware runs — handlers extract it the same way.
3. **Leader proxy** (`forward_to_leader`) uses `reqwest::Client` (already framework-agnostic) — only the Axum `Request`/`Response` wrapper types need updating to Actix equivalents.
4. **Inline permission checks** in admin handlers (`check_permission`) remain inline — they are not middleware layers and don't change structurally.

---

## Dependencies

```toml
# Remove
axum = ...
tower-http = ...

# Add
actix-web = "4"
actix-cors = "0.7"
actix-files = "0.6"          # replaces tower-http fs / ServeDir
actix-web-lab = "0.20"       # for from_fn middleware helper (or actix-web 4.4+)
```

`reqwest`, `tokio`, `serde`, `serde_json`, `tracing`, `bytes`, `base64`, `chrono` — all unchanged.

---

## Verification

1. `cargo build -p server-http` compiles cleanly
2. `cargo test -p server-http` passes
3. Start a single-node cluster: `PUT /cache/test/key` → 200, `GET /cache/test/key` → correct value
4. Start a 3-node cluster: send writes to all three nodes, verify follower forwarding works
5. Auth: unauthenticated request → 401, wrong permission → 403, valid token → 200
6. SSE: `GET /events` streams events when a cache key is written
7. Static UI: `GET /admin/ui/` serves files
8. Run a local benchmark (`wrk` or `oha`) against `GET /health` before and after to confirm throughput improvement
