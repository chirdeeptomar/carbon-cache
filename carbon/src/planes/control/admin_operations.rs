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
pub struct CacheManager<K, V>
where
    K: Debug + Hash + Eq + Send + Sync + 'static,
    V: Debug + Send + Sync + Clone + 'static,
{
    // Maps cache name -> storage implementation
    caches: Arc<RwLock<HashMap<String, Arc<dyn CacheStore<K, V>>>>>,
    _phantom: std::marker::PhantomData<(K, V)>,
}

impl<K, V> std::fmt::Debug for CacheManager<K, V>
where
    K: Debug + Hash + Eq + Send + Sync + 'static,
    V: Debug + Send + Sync + Clone + 'static,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CacheManager")
            .field("caches", &"<RwLock<HashMap>>")
            .finish()
    }
}

impl<K, V> Clone for CacheManager<K, V>
where
    K: Debug + Hash + Eq + Send + Sync + 'static,
    V: Debug + Send + Sync + Clone + 'static,
{
    fn clone(&self) -> Self {
        Self {
            caches: self.caches.clone(),
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<K, V> CacheManager<K, V>
where
    K: Debug + Hash + Eq + Send + Sync + 'static,
    V: Debug + Send + Sync + Clone + 'static,
{
    pub fn new() -> Self {
        Self {
            caches: Arc::new(RwLock::new(HashMap::new())),
            _phantom: std::marker::PhantomData,
        }
    }

    /// Get a cache by name
    pub async fn get_cache(&self, name: &str) -> Option<Arc<dyn CacheStore<K, V>>> {
        let caches = self.caches.read().await;
        caches.get(name).cloned()
    }

    /// Register a cache with a storage implementation
    pub async fn register_cache(
        &self,
        name: String,
        store: Arc<dyn CacheStore<K, V>>,
    ) -> Result<()> {
        let mut caches = self.caches.write().await;
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
            let caches = self.caches.read().await;
            if caches.contains_key(&config.name) {
                return Ok(CreateCacheResponse::new(
                    false,
                    format!("Cache '{}' already exists", config.name),
                ));
            }
        } // Read lock automatically released here

        // Note: Actual Foyer cache creation happens in the adapter layer (storage-engine)
        // The gRPC server should inject the cache store implementation
        // For now, we just track that the cache was requested

        // TODO: This should be injected from the gRPC layer
        // Example: let foyer_cache = FoyerCache::new(config.mem_bytes as usize);
        //          self.register_cache(config.name.clone(), Arc::new(foyer_cache)).await?;

        Ok(CreateCacheResponse::new(
            true,
            format!(
                "Cache '{}' configuration accepted. Use register_cache() to add the storage implementation.",
                config.name
            ),
        ))
    }

    async fn drop_cache(&self, name: &str) -> Result<DropCacheResponse> {
        let mut caches = self.caches.write().await;
        let dropped = caches.remove(name).is_some();
        Ok(DropCacheResponse::new(dropped))
    }

    async fn list_caches(&self) -> Result<ListCachesResponse> {
        let caches = self.caches.read().await;
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
        let caches = self.caches.read().await;
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
