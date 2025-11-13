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

/// CacheManager orchestrates cache operations using injected storage implementations
#[derive(Clone)]
pub struct CacheManager<K, V>
where
    K: Debug + Hash + Eq + Send + Sync + 'static,
    V: Debug + Send + Sync + 'static,
{
    // Maps cache name -> storage implementation
    cache_registry: Arc<RwLock<HashMap<String, Arc<dyn CacheStore<K, V>>>>>,
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

    /// Get a cache by name
    pub async fn get_cache(&self, name: &str) -> Option<Arc<dyn CacheStore<K, V>>> {
        let caches = self.cache_registry.read().await;
        caches.get(name).cloned()
    }

    /// Register a cache with a storage implementation
    pub async fn register_cache(
        &self,
        name: String,
        store: Arc<dyn CacheStore<K, V>>,
    ) -> Result<()> {
        let mut caches = self.cache_registry.write().await;
        caches.insert(name, store);
        Ok(())
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
impl<K, V> AdminOperations for CacheManager<K, V>
where
    K: Debug + Hash + Eq + Send + Sync + 'static,
    V: Debug + Send + Sync + Clone + 'static,
{
    async fn create_cache(&self, config: CacheConfig) -> Result<CreateCacheResponse> {
        // Check if cache already exists
        {
            let caches = self.cache_registry.read().await;
            if caches.contains_key(&config.name) {
                return Ok(CreateCacheResponse::new(
                    false,
                    format!("Cache '{}' already exists", config.name),
                ));
            }
        } // Read lock automatically released here

        Ok(CreateCacheResponse::new(
            true,
            format!(
                "Cache '{}' configuration accepted. Use register_cache() to add the storage implementation.",
                config.name
            ),
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
            .keys()
            .map(|name| {
                CacheInfo::from_config(crate::domain::CacheConfig::new(
                    name,
                    0,
                    None,
                    0,
                    crate::domain::EvictionPolicy::Unspecified,
                    None,
                    None,
                ))
            })
            .collect();
        Ok(ListCachesResponse::new(cache_infos))
    }

    async fn describe_cache(&self, name: &str) -> Result<DescribeCacheResponse> {
        let caches = self.cache_registry.read().await;
        if caches.contains_key(name) {
            Ok(DescribeCacheResponse::new(CacheInfo::from_config(
                crate::domain::CacheConfig::new(
                    name,
                    0,
                    None,
                    0,
                    crate::domain::EvictionPolicy::Unspecified,
                    None,
                    None,
                ),
            )))
        } else {
            Err(shared::Error::CacheNotFound(name.to_string()))
        }
    }
}
