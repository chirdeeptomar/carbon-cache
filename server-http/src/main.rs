mod handlers;
mod models;
mod routes;
mod state;

use state::AppState;
use tracing::{info, Level};

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    info!("Starting Carbon HTTP Server...");

    // Initialize state
    let state = AppState::new().await;

    // Build router
    let router = routes::build_router(state);

    // Start server
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080")
        .await
        .unwrap();

    info!("HTTP Server listening on http://0.0.0.0:8080");
    info!("Try: curl http://localhost:8080/health");

    axum::serve(listener, router).await.unwrap();
}
