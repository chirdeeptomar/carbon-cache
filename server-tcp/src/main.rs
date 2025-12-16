mod protocol;
mod server;

use server::process_connection;

use bytes::Bytes;
use std::sync::Arc;

use carbon::{
    planes::data::cache_operations::CacheOperationsService,
    planes::control::CacheManager,
};
use tokio::net::TcpListener;
use tracing::{Level, info};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    const DEFAULT_HOST: &str = "127.0.0.1";
    const DEFAULT_PORT: &str = "5500";

    // Initialize tracing
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    info!("Starting TCP server on {}:{}", DEFAULT_HOST, DEFAULT_PORT);

    // Load environment variables from .env file (if exists)
    match dotenvy::dotenv() {
        Ok(_) => info!("Loaded environment variables from .env file"),
        Err(_) => info!("No .env file found, using system environment variables"),
    }

    // Initialize CacheManager and CacheOperations
    let cache_manager = CacheManager::<Vec<u8>, Bytes>::new();
    let cache_ops = Arc::new(CacheOperationsService::new(cache_manager));

    let listener = TcpListener::bind(format!("{}:{}", DEFAULT_HOST, DEFAULT_PORT)).await?;

    info!(
        "TCP Server listening on tcp://{}:{}",
        DEFAULT_HOST, DEFAULT_PORT
    );

    loop {
        let (socket, addr) = listener.accept().await?;
        let cache_ops_clone = cache_ops.clone();
        tokio::spawn(async move {
            tracing::info!("Connection {addr} successful.");

            if let Err(err) = process_connection(socket, cache_ops_clone).await {
                tracing::warn!("Connection {addr} error: {err:?}");
            }
        });
    }
}
