use carbon::planes::control::CacheManager;
use carbon::planes::data::CacheOperationsService;

/// Server state shared across handlers
#[derive(Clone)]
pub struct AppState {
    pub cache_manager: CacheManager<Vec<u8>, Vec<u8>>,
    pub cache_operations: CacheOperationsService<Vec<u8>, Vec<u8>>,
}

impl AppState {
    pub fn new() -> Self {
        let cache_manager = CacheManager::new();
        let cache_operations = CacheOperationsService::new(cache_manager.clone());

        Self {
            cache_manager,
            cache_operations,
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
