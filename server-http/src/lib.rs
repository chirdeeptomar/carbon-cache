pub mod api;
pub mod handlers;
pub mod middleware;
pub mod routes;
pub mod state;
pub mod validation;

// Re-export key types
pub use state::AppState;
pub use routes::build_router;
