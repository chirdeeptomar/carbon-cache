use bytes::Bytes;
use carbon::auth::{AuthService, MokaSessionRepository, RoleService, SessionStore, UserService};
use carbon::events::CacheItemEvent;
use carbon::planes::control::CacheManager;
use carbon::planes::data::CacheOperationsService;
use std::sync::Arc;
use storage_engine::UnifiedStorageFactory;
use tokio::sync::broadcast;

/// Server state shared across handlers
#[derive(Clone)]
pub struct AppState {
    pub cache_manager: CacheManager<Vec<u8>, Bytes>,
    pub cache_operations: Arc<CacheOperationsService<Vec<u8>, Bytes>>,
    pub event_channel: broadcast::Sender<CacheItemEvent>,
    pub auth_service: Arc<AuthService>,
    pub user_service: Arc<UserService>,
    pub role_service: Arc<RoleService>,
    pub session_store: Arc<SessionStore<MokaSessionRepository>>,
}

impl AppState {
    pub async fn new(
        auth_service: Arc<AuthService>,
        user_service: Arc<UserService>,
        role_service: Arc<RoleService>,
        session_store: Arc<SessionStore<MokaSessionRepository>>,
    ) -> Self {
        // Try to initialize with persistence, fall back to in-memory if it fails
        let cache_manager = match Self::init_with_persistence().await {
            Ok(manager) => {
                tracing::info!("CacheManager initialized with persistence enabled");
                manager
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to initialize persistence: {}. Running in-memory mode.",
                    e
                );
                CacheManager::new()
            }
        };

        // Create broadcast channel for SSE events (1000 event buffer capacity)
        let (event_tx, _event_rx) = broadcast::channel(1000);

        // Create cache operations service with event broadcaster
        let cache_operations = Arc::new(CacheOperationsService::with_event_broadcaster(
            cache_manager.clone(),
            event_tx.clone(),
        ));

        Self {
            cache_manager,
            cache_operations,
            event_channel: event_tx,
            auth_service,
            user_service,
            role_service,
            session_store,
        }
    }

    async fn init_with_persistence() -> shared::Result<CacheManager<Vec<u8>, Bytes>> {
        // Get home directory for persistence path
        let home_dir = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_else(|_| ".".to_string());

        let persistence_path = std::path::Path::new(&home_dir)
            .join(".carbon")
            .join("caches.sled");

        // Create unified storage factory (supports Moka, Foyer Memory, and Foyer Hybrid)
        let factory = Arc::new(UnifiedStorageFactory);

        // Initialize CacheManager with persistence
        CacheManager::new_with_persistence(persistence_path, factory).await
    }
}
