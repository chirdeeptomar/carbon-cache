use carbon::auth::{
    defaults::create_default_admin, AuthService, MokaSessionRepository, RoleService, SessionStore,
    SledRoleRepository, SledUserRepository, UserRepository, UserService,
};
use carbon::planes::data::cache_operations::CacheOperationsService;
use shared::config::Config;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tracing::{info, warn, Level};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    info!("Starting Carbon Server");

    // Load environment variables
    match dotenvy::dotenv() {
        Ok(_) => info!("Loaded environment variables from .env file"),
        Err(_) => info!("No .env file found, using system environment variables"),
    }

    let config = Arc::new(Config::from_env());

    // ============================================
    // STEP 1: Initialize shared CacheManager
    // ============================================
    info!("Initializing shared CacheManager with persistence");

    let cache_manager = match server_http::AppState::init_with_persistence().await {
        Ok(cm) => {
            info!("CacheManager initialized with persistence enabled");
            cm
        }
        Err(e) => {
            warn!(
                "Failed to initialize persistence: {}. Running in-memory mode.",
                e
            );
            carbon::planes::control::CacheManager::new()
        }
    };

    let cache_ops = Arc::new(CacheOperationsService::new(cache_manager.clone()));

    // ============================================
    // STEP 2: Initialize Auth System
    // ============================================
    info!("Initializing authentication system...");
    let (auth_service, user_service, role_service) = init_auth_system(
        &config.data_dir,
        &config.admin_username,
        &config.admin_password,
    )
    .await;

    // Initialize session store (1 hour TTL)
    info!("Initializing session store...");
    let session_repository = Arc::new(MokaSessionRepository::new(
        None,                            // No max sessions limit
        Some(Duration::from_secs(3600)), // 1 hour TTL
    ));
    let session_store = Arc::new(SessionStore::new(session_repository));

    // ============================================
    // STEP 3: Initialize HTTP Server State
    // ============================================
    info!("Initializing HTTP server components");

    let app_state = server_http::AppState::new_with_cache_manager(
        cache_manager,
        auth_service,
        user_service,
        role_service,
        session_store,
    )
    .await;

    let http_router = server_http::build_router(app_state, &config);

    // ============================================
    // STEP 4: Spawn HTTP Server Task
    // ============================================
    let config_http_server = Arc::clone(&config);

    let http_handle = tokio::spawn(async move {
        info!(
            "Starting HTTP server on {}://{}:{}",
            config_http_server.http.http_protcol(),
            config_http_server.host,
            config_http_server.http.port()
        );

        let listener = TcpListener::bind(format!(
            "{}:{}",
            config_http_server.host,
            config_http_server.http.port()
        ))
        .await
        .expect("Failed to bind HTTP server");

        info!(
            "HTTP Server listening on {}://{}:{}",
            config_http_server.http.http_protcol(),
            config_http_server.host,
            config_http_server.http.port()
        );

        info!(
            "Try: curl -u admin:admin123 {}://{}:{}/health",
            config_http_server.http.http_protcol(),
            config_http_server.host,
            config_http_server.http.port()
        );

        axum::serve(listener, http_router)
            .with_graceful_shutdown(shutdown_signal())
            .await
            .expect("HTTP server error");
    });

    // ============================================
    // STEP 5: Spawn TCP Server Task
    // ============================================
    info!("Initializing TCP server components");

    let config_tcp_server = Arc::clone(&config);

    let tcp_cache_ops = cache_ops.clone();

    let tcp_handle = tokio::spawn(async move {
        info!(
            "Starting TCP server on {}://{}:{}",
            config_tcp_server.tcp.tcp_protcol(),
            config_tcp_server.host,
            config_tcp_server.tcp.port()
        );

        let listener = TcpListener::bind(format!(
            "{}:{}",
            config_tcp_server.host,
            config_tcp_server.tcp.port()
        ))
        .await
        .expect("Failed to bind TCP server");

        info!(
            "TCP Server listening on {}://{}:{}",
            config_tcp_server.tcp.tcp_protcol(),
            config_tcp_server.host,
            config_tcp_server.tcp.port()
        );

        loop {
            match listener.accept().await {
                Ok((socket, addr)) => {
                    tracing::info!("TCP connection from {addr}");
                    let cache_ops_clone = tcp_cache_ops.clone();

                    tokio::spawn(async move {
                        if let Err(err) =
                            server_tcp::process_connection(socket, cache_ops_clone).await
                        {
                            tracing::warn!("TCP connection {addr} error: {err:?}");
                        }
                    });
                }
                Err(e) => {
                    tracing::error!("TCP accept error: {}", e);
                }
            }
        }
    });

    // ============================================
    // STEP 6: Wait for shutdown signal
    // ============================================
    info!("Carbon server started successfully");
    info!(
        "  - HTTP: {}://{}:{}",
        config.http.http_protcol(),
        config.host,
        config.http.port()
    );
    info!(
        "  - TCP:  {}://{}:{}",
        config.tcp.tcp_protcol(),
        config.host,
        config.tcp.port()
    );

    tokio::select! {
        _ = http_handle => info!("HTTP server task completed"),
        _ = tcp_handle => info!("TCP server task completed"),
        _ = shutdown_signal() => info!("Shutdown signal received"),
    }

    info!("Carbon server shutting down");
    Ok(())
}

// Graceful shutdown handler
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

// Initialize authentication system
async fn init_auth_system(
    data_dir: &str,
    admin_username: &str,
    admin_password: &str,
) -> (Arc<AuthService>, Arc<UserService>, Arc<RoleService>) {
    let auth_base_path = std::path::Path::new(&data_dir).join(".carbon");
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

    let admin_exists = user_repo
        .username_exists(admin_username)
        .await
        .unwrap_or(false);

    if !admin_exists {
        info!("Creating default admin user: {}", admin_username);
        let admin_user = create_default_admin(
            admin_username.to_string(),
            admin_password.to_string(),
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
