use crate::handlers;
use crate::state::AppState;
use axum::{
    routing::{delete, get, post, put},
    Router,
};
use tower_http::normalize_path::NormalizePathLayer;
use tower_http::trace::TraceLayer;

/// Build and configure the application router
pub fn build_router(state: AppState) -> Router {
    Router::new()
        // Health check
        .route("/health", get(handlers::health_check))
        // SSE Events endpoint
        .route("/events", get(handlers::stream_events))
        // Admin routes
        .route("/admin/caches", post(handlers::create_cache))
        .route("/admin/caches", get(handlers::list_caches))
        .route("/admin/caches/{name}", get(handlers::describe_cache))
        .route("/admin/caches/{name}", delete(handlers::drop_cache))
        // Cache operation routes
        .route("/cache/{cache_name}/{key}", put(handlers::put_value))
        .route("/cache/{cache_name}/{key}", get(handlers::get_value))
        .route("/cache/{cache_name}/{key}", delete(handlers::delete_value))
        // Middleware
        .layer(NormalizePathLayer::trim_trailing_slash())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
