use bytes::Bytes;
use carbon::auth::{
    AuthService, MokaSessionRepository, RedbRoleRepository, RedbUserRepository, RoleRepository,
    RoleService, SessionStore, UserRepository, UserService, defaults::create_default_admin,
};
use carbon::planes::control::CacheManager;
use carbon::planes::control::operation::AdminOperations;
use carbon::planes::data::cache_operations::CacheOperationsService;
use carbon::planes::data::operation::CacheOperations;
use shared::config::Config;
use std::sync::Arc;
use std::time::Duration;
use storage_engine::UnifiedStorageFactory;
use tokio::net::TcpListener;
use tracing::{info, warn};

pub async fn start(config: Arc<Config>) -> Result<(), Box<dyn std::error::Error>> {
    info!("Mode: standalone (no Raft consensus)");

    // ── Cache ──────────────────────────────────────────────────────────────
    let cache_configs_path = std::path::Path::new(&config.data_dir)
        .join(".carbon")
        .join("caches.redb");
    let cache_manager = CacheManager::<Vec<u8>, Bytes>::new_with_persistence(
        cache_configs_path,
        Arc::new(UnifiedStorageFactory),
    )
    .await
    .expect("Failed to open cache config store");
    let admin_ops: Arc<dyn AdminOperations<Vec<u8>, Bytes>> = Arc::new(cache_manager.clone());
    let cache_ops: Arc<dyn CacheOperations<Vec<u8>, Bytes>> =
        Arc::new(CacheOperationsService::new(cache_manager));

    // ── Auth ───────────────────────────────────────────────────────────────
    let auth_base = std::path::Path::new(&config.data_dir).join(".carbon");
    std::fs::create_dir_all(&auth_base)?;

    let user_repo = Arc::new(
        RedbUserRepository::new(auth_base.join("users.redb")).expect("Failed to open users.redb"),
    );
    let role_repo = Arc::new(
        RedbRoleRepository::new(auth_base.join("roles.redb")).expect("Failed to open roles.redb"),
    ) as Arc<dyn RoleRepository>;

    let auth_service = Arc::new(AuthService::new(
        user_repo.clone() as Arc<dyn UserRepository>,
        role_repo.clone(),
    ));
    let user_service = Arc::new(UserService::new(
        user_repo.clone() as Arc<dyn UserRepository>,
        role_repo.clone(),
    ));
    let role_service = Arc::new(RoleService::new(role_repo.clone()));

    init_auth_defaults(
        &user_repo,
        &role_repo,
        &config.admin_username,
        &config.admin_password,
    )
    .await;

    let session_repository = Arc::new(MokaSessionRepository::new(
        None,
        Some(Duration::from_secs(3600)),
    ));
    let session_store = Arc::new(SessionStore::new(session_repository));

    // ── HTTP ───────────────────────────────────────────────────────────────
    let app_state = server_http::AppState::new(
        cache_ops,
        admin_ops,
        None,
        auth_service,
        user_service,
        role_service,
        session_store,
    );

    let tcp_cache_ops = app_state.cache_ops.clone();

    let http_router = server_http::build_router(app_state, &config);

    // ── TCP ────────────────────────────────────────────────────────────────
    let config_tcp = Arc::clone(&config);

    let tcp_handle = tokio::spawn(async move {
        info!(
            "Starting TCP server on {}:{}",
            config_tcp.host,
            config_tcp.tcp.port()
        );
        let listener = TcpListener::bind(format!("{}:{}", config_tcp.host, config_tcp.tcp.port()))
            .await
            .expect("Failed to bind TCP server");
        info!(
            "TCP server listening on {}:{}",
            config_tcp.host,
            config_tcp.tcp.port()
        );
        loop {
            match listener.accept().await {
                Ok((socket, addr)) => {
                    let ops = tcp_cache_ops.clone();
                    tokio::spawn(async move {
                        if let Err(e) = server_tcp::process_connection(socket, ops, None).await {
                            tracing::warn!("TCP {addr} error: {e:?}");
                        }
                    });
                }
                Err(e) => tracing::error!("TCP accept error: {e}"),
            }
        }
    });

    let config_http = Arc::clone(&config);
    let http_handle = tokio::spawn(async move {
        info!(
            "Starting HTTP server on {}:{}",
            config_http.host,
            config_http.http.port()
        );
        let listener =
            TcpListener::bind(format!("{}:{}", config_http.host, config_http.http.port()))
                .await
                .expect("Failed to bind HTTP server");
        info!(
            "HTTP server listening on {}://{}:{}",
            config_http.http.http_protcol(),
            config_http.host,
            config_http.http.port()
        );
        axum::serve(listener, http_router)
            .with_graceful_shutdown(shutdown_signal())
            .await
            .expect("HTTP server error");
    });

    info!("Carbon standalone server started");
    info!(
        "  HTTP : {}://{}:{}",
        config.http.http_protcol(),
        config.host,
        config.http.port()
    );
    info!("  TCP  : {}:{}", config.host, config.tcp.port());

    tokio::select! {
        _ = http_handle => info!("HTTP server task completed"),
        _ = tcp_handle  => info!("TCP server task completed"),
        _ = shutdown_signal() => info!("Shutdown signal received"),
    }

    info!("Carbon standalone server shutting down");
    Ok(())
}

async fn init_auth_defaults(
    user_repo: &Arc<RedbUserRepository>,
    role_repo: &Arc<dyn RoleRepository>,
    admin_username: &str,
    admin_password: &str,
) {
    let role_service = RoleService::new(role_repo.clone());
    let default_roles = match role_service.initialize_default_roles().await {
        Ok(r) => r,
        Err(e) => {
            warn!("Failed to initialize default roles: {e}");
            return;
        }
    };

    let admin_role_id = default_roles
        .iter()
        .find(|r| r.name == "admin")
        .map(|r| r.id.clone())
        .unwrap_or_default();

    let exists = user_repo
        .username_exists(admin_username)
        .await
        .unwrap_or(false);
    if !exists {
        match create_default_admin(
            admin_username.to_string(),
            admin_password.to_string(),
            admin_role_id,
        ) {
            Ok(user) => {
                if let Err(e) = user_repo.create(user).await {
                    warn!("Failed to create default admin: {e}");
                } else {
                    info!("Created default admin user: {admin_username}");
                }
            }
            Err(e) => warn!("Failed to hash admin password: {e}"),
        }
    } else {
        info!("Admin user already exists: {admin_username}");
    }
}

async fn shutdown_signal() {
    use tokio::signal;
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler")
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
        _ = ctrl_c => info!("Received Ctrl+C"),
        _ = terminate => info!("Received terminate"),
    }
}
