use crate::domain::response::admin::{
    CreateCacheResponse, DescribeCacheResponse, DropCacheResponse, ListCachesResponse,
};
use crate::domain::{CacheConfig, CacheInfo};

use crate::ports::{AdminOperations, CacheStore};
use async_trait::async_trait;
use shared::Result;
use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::Hash;
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
    K: Debug + Hash + Eq + Send + Sync + 'static,
    V: Debug + Send + Sync + 'static,
{
    pub fn new() -> Self {
        Self {
            cache_registry: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get a cache store by name
    pub async fn get_cache_store(&self, name: &str) -> Option<Arc<dyn CacheStore<K, V>>> {
        let caches = self.cache_registry.read().await;
        caches.get(name).map(|entry| entry.store.clone())
    }

}

impl<K, V> Default for CacheManager<K, V>
where
    K: Debug + Hash + Eq + Send + Sync + 'static,
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
        Ok(DropCacheResponse::new(dropped))
    }

    async fn list_caches(&self) -> Result<ListCachesResponse> {
        let caches = self.cache_registry.read().await;
        let cache_infos: Vec<CacheInfo> = caches
            .values()
            .map(|entry| CacheInfo::from_config(entry.config.clone()))
            .collect();
        Ok(ListCachesResponse::new(cache_infos))
    }

    async fn describe_cache(&self, name: &str) -> Result<DescribeCacheResponse> {
        let caches = self.cache_registry.read().await;
        if let Some(entry) = caches.get(name) {
            Ok(DescribeCacheResponse::new(CacheInfo::from_config(
                entry.config.clone(),
            )))
        } else {
            Err(shared::Error::CacheNotFound(name.to_string()))
        }
    }
}
