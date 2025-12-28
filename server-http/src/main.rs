mod api;
mod handlers;
mod middleware;
mod routes;
mod state;
mod validation;

use carbon::auth::{
    defaults::create_default_admin, AuthService, MokaSessionRepository, RoleService, SessionStore,
    SledRoleRepository, SledUserRepository, UserRepository, UserService,
};
use shared::config::{self, Config};
use state::AppState;
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn, Level};

#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

#[tokio::main]
async fn main() {
    let _profiler = dhat::Profiler::new_heap();

    // Initialize tracing
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    info!("Starting Carbon HTTP Server...");

    // Load environment variables from .env file (if exists)
    match dotenvy::dotenv() {
        Ok(_) => info!("Loaded environment variables from .env file"),
        Err(_) => info!("No .env file found, using system environment variables"),
    }

    // Load configuration from environment variables
    let config = Arc::new(config::Config::from_env());

    // Initialize auth system
    info!("Initializing authentication system...");
    let (auth_service, user_service, role_service) = init_auth_system(&config).await;

    // Initialize session store (1 hour TTL)
    info!("Initializing session store...");
    let session_repository = Arc::new(MokaSessionRepository::new(
        None,                            // No max sessions limit
        Some(Duration::from_secs(3600)), // 1 hour TTL
    ));
    let session_store = Arc::new(SessionStore::new(session_repository));

    // Initialize state
    let state = AppState::new(auth_service, user_service, role_service, session_store).await;

    // Build router
    let router = routes::build_router(state, &config);

    // Start server
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();

    info!("HTTP Server listening on http://0.0.0.0:8080");
    info!("Try: curl -u admin:admin123 http://localhost:8080/health");

    // Graceful shutdown handler
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();

    info!("Server shutdown complete. Writing dhat profiling data...");
    drop(_profiler); // Explicitly drop profiler to write output
    info!("Profiling data written to dhat-heap.json");
}

async fn shutdown_signal() {
    use tokio::signal;

    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

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
        _ = ctrl_c => {
            info!("Received Ctrl+C signal");
        },
        _ = terminate => {
            info!("Received terminate signal");
        },
    }

    info!("Shutting down gracefully...");
}

async fn init_auth_system(
    config: &Arc<Config>,
) -> (Arc<AuthService>, Arc<UserService>, Arc<RoleService>) {
    // Get home directory for auth storage
    let home_dir = &config.data_dir;

    let auth_base_path = std::path::Path::new(&home_dir).join(".carbon");

    // Create .carbon directory if it doesn't exist
    if let Err(e) = std::fs::create_dir_all(&auth_base_path) {
        warn!("Failed to create .carbon directory: {}", e);
    }

    // Initialize repositories
    let user_repo = Arc::new(
        SledUserRepository::new(auth_base_path.join("users.sled"))
            .expect("Failed to initialize user repository"),
    );
    let role_repo = Arc::new(
        SledRoleRepository::new(auth_base_path.join("roles.sled"))
            .expect("Failed to initialize role repository"),
    );

    // Initialize services
    let auth_service = Arc::new(AuthService::new(user_repo.clone(), role_repo.clone()));
    let user_service = Arc::new(UserService::new(user_repo.clone(), role_repo.clone()));
    let role_service = Arc::new(RoleService::new(role_repo.clone()));

    // Initialize default roles
    info!("Initializing default roles...");
    let default_roles = role_service
        .initialize_default_roles()
        .await
        .expect("Failed to initialize default roles");

    info!("Default roles created: admin, user, read-only");

    // Get admin role ID
    let admin_role = default_roles
        .iter()
        .find(|r| r.name == "admin")
        .expect("Admin role not found");

    // Check if default admin exists
    let admin_username = &config.admin_username;
    let admin_exists = user_repo
        .username_exists(admin_username)
        .await
        .unwrap_or(false);

    if !admin_exists {
        // Create default admin user
        let admin_password = &config.admin_password;

        info!("Creating default admin user: {}", admin_username);
        let admin_user = create_default_admin(
            admin_username.clone(),
            admin_password.clone(),
            admin_role.id.clone(),
        )
        .expect("Failed to create default admin user");

        user_repo
            .create(admin_user)
            .await
            .expect("Failed to save default admin user");

        info!("✓ Default admin user created: {}", admin_username);
    } else {
        info!("Admin user already exists: {}", admin_username);
    }

    (auth_service, user_service, role_service)
}
