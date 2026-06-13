use bytes::Bytes;
use carbon::auth::{
    AuthService, MokaSessionRepository, RoleRepository, RoleService, SessionStore, UserRepository,
    UserService,
    defaults::{create_default_admin, create_default_roles},
};
use carbon::planes::control::operation::AdminOperations;
use carbon::planes::data::operation::CacheOperations;
use carbon_raft::log_store::CarbonRaftStorage;
use carbon_raft::node::RaftCacheNode;
use carbon_raft::types::RaftLogEntry;
use carbon_raft::{RaftRoleRepository, RaftUserRepository};
use shared::config::Config;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tracing::{info, warn};

pub async fn start(config: Arc<Config>) -> Result<(), Box<dyn std::error::Error>> {
    info!(
        "Mode: cluster (Raft consensus) — node_id={}, raft_addr={}, seeds={:?}",
        config.node_id, config.raft_addr, config.seeds
    );

    // ── Raft node ──────────────────────────────────────────────────────────
    std::fs::create_dir_all(&config.cluster_data_dir)?;
    let db_path = std::path::Path::new(&config.cluster_data_dir).join("raft.redb");
    let storage = CarbonRaftStorage::open(db_path)?;
    let raft_node = RaftCacheNode::new(
        config.node_id,
        config.raft_addr.clone(),
        config.http_addr.clone(),
        config.tcp_addr.clone(),
        config.seeds.clone(),
        storage,
    )
    .await?;

    info!("Raft node started");

    // ── Auth (Raft-backed) ─────────────────────────────────────────────────
    let session_repository = Arc::new(MokaSessionRepository::new(
        None,
        Some(Duration::from_secs(3600)),
    ));
    let session_store = Arc::new(SessionStore::new(session_repository));

    let user_repo = Arc::new(RaftUserRepository {
        sm: raft_node.state.clone(),
    }) as Arc<dyn UserRepository>;
    let role_repo = Arc::new(RaftRoleRepository {
        sm: raft_node.state.clone(),
    }) as Arc<dyn RoleRepository>;

    let auth_service = Arc::new(AuthService::new(user_repo.clone(), role_repo.clone()));
    let user_service = Arc::new(UserService::new(user_repo.clone(), role_repo.clone()));
    let role_service = Arc::new(RoleService::new(role_repo));

    let node_for_defaults = raft_node.clone();
    let admin_username = config.admin_username.clone();
    let admin_password = config.admin_password.clone();
    tokio::spawn(async move {
        init_raft_auth_defaults(node_for_defaults, admin_username, admin_password).await;
    });

    // ── HTTP ───────────────────────────────────────────────────────────────
    let cache_ops: Arc<dyn CacheOperations<Vec<u8>, Bytes>> = raft_node.clone();
    let admin_ops: Arc<dyn AdminOperations<Vec<u8>, Bytes>> = raft_node.clone();
    let app_state = server_http::AppState::new(
        cache_ops,
        admin_ops,
        Some(raft_node.clone()),
        auth_service,
        user_service,
        role_service,
        session_store,
    );

    let http_router = server_http::build_router(app_state, &config);

    // ── TCP ────────────────────────────────────────────────────────────────
    let config_tcp = Arc::clone(&config);
    let tcp_node = raft_node.clone();

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
                    let ops: Arc<dyn CacheOperations<Vec<u8>, Bytes>> = tcp_node.clone();
                    let raft = tcp_node.clone();
                    tokio::spawn(async move {
                        if let Err(e) = server_tcp::process_connection(socket, ops, Some(raft)).await {
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

    info!("Carbon cluster server started");
    info!(
        "  HTTP : {}://{}:{}",
        config.http.http_protcol(),
        config.host,
        config.http.port()
    );
    info!("  TCP  : {}:{}", config.host, config.tcp.port());
    info!("  Raft RPC : {}", config.raft_addr);

    tokio::select! {
        _ = http_handle => info!("HTTP server task completed"),
        _ = tcp_handle  => info!("TCP server task completed"),
        _ = shutdown_signal() => info!("Shutdown signal received"),
    }

    info!("Carbon cluster server shutting down");
    Ok(())
}

async fn init_raft_auth_defaults(
    node: Arc<RaftCacheNode>,
    admin_username: String,
    admin_password: String,
) {
    for _ in 0..40 {
        tokio::time::sleep(Duration::from_millis(250)).await;
        if node.raft.metrics().borrow().current_leader.is_some() {
            break;
        }
    }

    let metrics = node.raft.metrics().borrow().clone();
    let Some(leader_id) = metrics.current_leader else {
        warn!("No Raft leader elected — skipping auth default bootstrap");
        return;
    };
    if leader_id != metrics.id {
        info!("Not the leader — skipping auth defaults (leader is node {leader_id})");
        return;
    }

    info!("Leader elected — bootstrapping default roles and admin user via Raft");

    let default_roles = create_default_roles();
    let mut admin_role_id = String::new();

    for role in default_roles {
        if role.name == "admin" {
            admin_role_id = role.id.clone();
        }
        if node.get_role_by_name(&role.name).await.is_none() {
            match node.write(RaftLogEntry::CreateRole(role.clone())).await {
                Ok(_) => info!("Created default role: {}", role.name),
                Err(e) => warn!("Failed to create default role {}: {e}", role.name),
            }
        }
    }

    if admin_role_id.is_empty()
        && let Some(r) = node.get_role_by_name("admin").await
    {
        admin_role_id = r.id;
    }

    if !node.username_exists(&admin_username).await {
        match create_default_admin(admin_username.clone(), admin_password, admin_role_id) {
            Ok(user) => match node.write(RaftLogEntry::CreateUser(user)).await {
                Ok(_) => info!("Created default admin user: {admin_username}"),
                Err(e) => warn!("Failed to write admin user to Raft: {e}"),
            },
            Err(e) => warn!("Failed to hash admin password: {e}"),
        }
    } else {
        info!("Admin user already exists in cluster state");
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
