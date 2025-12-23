use crate::handlers;
use crate::middleware::{auth_middleware, AuthMiddlewareState};
use crate::state::AppState;
use axum::{
    middleware,
    routing::{delete, get, post, put},
    Router,
};
use tower_http::normalize_path::NormalizePathLayer;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;

/// Build and configure the application router
pub fn build_router(state: AppState) -> Router {
    // Public routes (no authentication required)
    let public_routes = Router::new()
        .route("/health", get(handlers::health_check))
        .with_state(state.clone());

    // Auth routes (login/logout endpoints)
    let auth_state = handlers::AuthHandlerState {
        auth_service: state.auth_service.clone(),
        session_store: state.session_store.clone(),
    };

    let auth_routes = Router::new()
        // Add route for admin UI
        .nest_service("/admin/ui", ServeDir::new("../carbon-admin-ui/dist"))
        .route("/auth/login", post(handlers::login))
        .route("/auth/logout", post(handlers::logout))
        .with_state(auth_state);

    // Create auth middleware state
    let auth_state = AuthMiddlewareState {
        auth_service: state.auth_service.clone(),
        session_store: state.session_store.clone(),
    };

    // Protected routes (authentication required)
    let protected_routes = Router::new()
        // SSE Events endpoint - requires ReadCache permission (checked in handler if needed)
        .route("/events", get(handlers::stream_events))
        // Cache operation routes - requires cache permissions (checked in handlers)
        .route("/cache/{cache_name}/{key}", put(handlers::put_value))
        .route("/cache/{cache_name}/{key}", get(handlers::get_value))
        .route("/cache/{cache_name}/{key}", delete(handlers::delete_value))
        // Admin cache routes - requires admin permissions (checked in handlers)
        .route("/admin/caches", post(handlers::create_cache))
        .route("/admin/caches", get(handlers::list_caches))
        .route("/admin/caches/{name}", get(handlers::describe_cache))
        .route("/admin/caches/{name}", delete(handlers::drop_cache))
        // User management routes - requires ManageUsers permission (checked in handlers)
        .route("/admin/users", post(handlers::create_user))
        .route("/admin/users", get(handlers::list_users))
        .route("/admin/users/{username}", get(handlers::get_user))
        .route("/admin/users/{username}/roles", put(handlers::assign_roles))
        .route(
            "/admin/users/{username}/password",
            put(handlers::change_password),
        )
        .route(
            "/admin/users/{username}/reset-password",
            put(handlers::reset_password),
        )
        .route("/admin/users/{username}", delete(handlers::delete_user))
        // Role management routes - requires ManageRoles/AdminRead permission (checked in handlers)
        .route("/admin/roles", post(handlers::create_role))
        .route("/admin/roles", get(handlers::list_roles))
        .route("/admin/roles/{name}", get(handlers::get_role))
        .route("/admin/roles/{name}", put(handlers::update_role))
        .route("/admin/roles/{name}", delete(handlers::delete_role))
        // Apply authentication middleware to all protected routes
        .layer(middleware::from_fn_with_state(auth_state, auth_middleware));

    // Combine routes
    Router::new()
        .merge(public_routes)
        .merge(auth_routes)
        .merge(protected_routes)
        .layer(NormalizePathLayer::trim_trailing_slash())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
