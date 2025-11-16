use crate::domain::response::admin::{
    CreateCacheResponse, DescribeCacheResponse, DropCacheResponse, ListCachesResponse,
};
use crate::domain::{CacheConfig, CacheInfo};
use crate::persistence::SledPersistence;
use crate::ports::{AdminOperations, CacheStore, StorageFactory};
use async_trait::async_trait;
use shared::Result;
use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::Hash;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Entry containing both cache configuration and storage implementation
pub struct CacheMetadata<K, V>
where
    K: Debug + Hash + Eq + Send + Sync + 'static,
    V: Debug + Send + Sync + 'static,
{
    pub config: CacheConfig,
    pub store: Arc<dyn CacheStore<K, V>>,
}

/// CacheManager orchestrates cache operations using injected storage implementations
#[derive(Clone)]
pub struct CacheManager<K, V>
where
    K: Debug + Hash + Eq + Send + Sync + 'static,
    V: Debug + Send + Sync + 'static,
{
    // Maps cache name -> cache metadata (config + storage implementation)
    cache_registry: Arc<RwLock<HashMap<String, CacheMetadata<K, V>>>>,
    // Optional persistence layer for cache configurations
    persistence: Option<Arc<SledPersistence>>,
}

impl<K, V> Debug for CacheManager<K, V>
where
    K: Debug + Hash + Eq + Send + Sync + 'static,
    V: Debug + Send + Sync + 'static,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CacheManager")
            .field("caches", &"<RwLock<HashMap>>")
            .finish()
    }
}

impl<K, V> CacheManager<K, V>
where
    K: Debug + Hash + Eq + Send + Sync + Clone + 'static,
    V: Debug + Send + Sync + Clone + 'static,
{
    /// Create a new CacheManager without persistence (in-memory only)
    pub fn new() -> Self {
        Self {
            cache_registry: Arc::new(RwLock::new(HashMap::new())),
            persistence: None,
        }
    }

    /// Create a new CacheManager with persistence enabled
    /// This will eagerly load all cached configurations from Sled and recreate the caches
    pub async fn new_with_persistence(
        persistence_path: impl AsRef<Path>,
        factory: Arc<dyn StorageFactory<K, V>>,
    ) -> Result<Self> {
        let persistence = SledPersistence::new(persistence_path)?;

        // Load all configs from persistence
        let configs = persistence.load_all()?;

        // Create manager
        let manager = Self {
            cache_registry: Arc::new(RwLock::new(HashMap::new())),
            persistence: Some(Arc::new(persistence)),
        };

        // Eagerly recreate all caches from configs (Option B)
        let mut caches = manager.cache_registry.write().await;
        for config in configs {
            let store = factory.create_from_config(&config);
            let cache_name = config.name.clone();
            let entry = CacheMetadata {
                config,
                store,
            };
            caches.insert(cache_name, entry);
        }
        drop(caches); // Release the lock

        Ok(manager)
    }

    /// Get a cache store by name
    pub async fn get_cache_store(&self, name: &str) -> Option<Arc<dyn CacheStore<K, V>>> {
        let caches = self.cache_registry.read().await;
        caches.get(name).map(|entry| entry.store.clone())
    }

}

impl<K, V> Default for CacheManager<K, V>
where
    K: Debug + Hash + Eq + Send + Sync + Clone + 'static,
    V: Debug + Send + Sync + Clone + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl<K, V> AdminOperations<K, V> for CacheManager<K, V>
where
    K: Debug + Hash + Eq + Send + Sync + 'static,
    V: Debug + Send + Sync + Clone + 'static,
{

    /// Create and register a cache with configuration and storage implementation (unified operation)
    async fn create_cache(
        &self,
        config: CacheConfig,
        store: Arc<dyn CacheStore<K, V>>,
    ) -> Result<CreateCacheResponse> {
        let mut caches = self.cache_registry.write().await;

        // Check if cache already exists
        if caches.contains_key(&config.name) {
            return Ok(CreateCacheResponse::new(
                false,
                format!("Cache '{}' already exists", config.name),
            ));
        }

        let cache_name = config.name.clone();

        // Persist to Sled if persistence is enabled
        if let Some(ref persistence) = self.persistence {
            persistence.save_config(&config)?;
        }

        let entry = CacheMetadata {
            config,
            store,
        };
        caches.insert(cache_name.clone(), entry);

        Ok(CreateCacheResponse::new(
            true,
            format!("Cache '{}' created successfully", cache_name),
        ))
    }

    async fn drop_cache(&self, name: &str) -> Result<DropCacheResponse> {
        let mut caches = self.cache_registry.write().await;
        let dropped = caches.remove(name).is_some();

        // Delete from Sled if persistence is enabled and cache was dropped
        if dropped {
            if let Some(ref persistence) = self.persistence {
                persistence.delete_config(name)?;
            }
        }

        Ok(DropCacheResponse::new(dropped))
    }

    async fn list_caches(&self) -> Result<ListCachesResponse> {
        let caches = self.cache_registry.read().await;
        let cache_infos: Vec<CacheInfo> = caches
            .values()
            .map(|entry| CacheInfo::from_config(&entry.config))
            .collect();
        Ok(ListCachesResponse::new(cache_infos))
    }

    async fn describe_cache(&self, name: &str) -> Result<DescribeCacheResponse> {
        let caches = self.cache_registry.read().await;
        if let Some(entry) = caches.get(name) {
            Ok(DescribeCacheResponse::new(CacheInfo::from_config(
                &entry.config,
            )))
        } else {
            Err(shared::Error::CacheNotFound(name.to_string()))
        }
    }
}
