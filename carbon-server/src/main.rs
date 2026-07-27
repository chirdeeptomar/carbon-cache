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
