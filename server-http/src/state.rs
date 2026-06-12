use bytes::Bytes;
use carbon::auth::{AuthService, MokaSessionRepository, RoleService, SessionStore, UserService};
use carbon::events::CacheItemEvent;
use carbon::planes::data::operation::CacheOperations;
use carbon_raft::node::RaftCacheNode;
use std::sync::Arc;
use tokio::sync::broadcast;

#[derive(Clone)]
pub struct AppState {
    pub event_channel: broadcast::Sender<CacheItemEvent>,
    pub auth_service: Arc<AuthService>,
    pub user_service: Arc<UserService>,
    pub role_service: Arc<RoleService>,
    pub session_store: Arc<SessionStore<MokaSessionRepository>>,
    pub cache_ops: Arc<dyn CacheOperations<Vec<u8>, Bytes>>,
    pub raft_node: Option<Arc<RaftCacheNode>>,
}

impl AppState {
    pub fn new(
        cache_ops: Arc<dyn CacheOperations<Vec<u8>, Bytes>>,
        raft_node: Option<Arc<RaftCacheNode>>,
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
            cache_ops,
            raft_node,
        }
    }
}
