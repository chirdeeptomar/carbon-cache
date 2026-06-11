use bytes::Bytes;
use carbon::auth::{AuthService, MokaSessionRepository, RoleService, SessionStore, UserService};
use carbon::events::CacheItemEvent;
use carbon_raft::node::RaftCacheNode;
use std::sync::Arc;
use tokio::sync::broadcast;

/// Server state shared across all HTTP handlers.
/// Raft is always active — single-node deployments self-elect as leader immediately.
#[derive(Clone)]
pub struct AppState {
    pub event_channel: broadcast::Sender<CacheItemEvent>,
    pub auth_service: Arc<AuthService>,
    pub user_service: Arc<UserService>,
    pub role_service: Arc<RoleService>,
    pub session_store: Arc<SessionStore<MokaSessionRepository>>,
    pub raft_node: Arc<RaftCacheNode>,
}

impl AppState {
    pub async fn new(
        raft_node: Arc<RaftCacheNode>,
        auth_service: Arc<AuthService>,
        user_service: Arc<UserService>,
        role_service: Arc<RoleService>,
        session_store: Arc<SessionStore<MokaSessionRepository>>,
    ) -> Self {
        let (event_tx, _event_rx) = broadcast::channel::<CacheItemEvent>(1000);

        Self {
            event_channel: event_tx,
            auth_service,
            user_service,
            role_service,
            session_store,
            raft_node,
        }
    }
}

// Suppress unused import warning — Bytes is kept for downstream handler compatibility
const _: fn() = || {
    let _: Bytes;
};
