use carbon::planes::control::CacheManager;
use carbon::planes::data::CacheOperationsService;
use storage_engine::UnifiedStorageFactory;
use std::sync::Arc;

/// Server state shared across handlers
#[derive(Clone)]
pub struct AppState {
    pub cache_manager: CacheManager<Vec<u8>, Vec<u8>>,
    pub cache_operations: CacheOperationsService<Vec<u8>, Vec<u8>>,
}

impl AppState {
    pub async fn new() -> Self {
        // Try to initialize with persistence, fall back to in-memory if it fails
        let cache_manager = match Self::init_with_persistence().await {
            Ok(manager) => {
                tracing::info!("CacheManager initialized with persistence enabled");
                manager
            }
            Err(e) => {
                tracing::warn!("Failed to initialize persistence: {}. Running in-memory mode.", e);
                CacheManager::new()
            }
        };

        let cache_operations = CacheOperationsService::new(cache_manager.clone());

        Self {
            cache_manager,
            cache_operations,
        }
    }

    async fn init_with_persistence() -> shared::Result<CacheManager<Vec<u8>, Vec<u8>>> {
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
