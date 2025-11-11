use crate::domain::response::{DeleteResponse, GetResponse, PutResponse};
use crate::planes::control::CacheManager;
use crate::planes::data::operation::CacheOperations;
use crate::ports::CacheStore;
use async_trait::async_trait;
use shared::{Error, Result, TtlMs};
use std::fmt::Debug;
use std::hash::Hash;
use std::sync::Arc;

/// Application service that orchestrates cache operations
/// This is the main entry point for all cache operations in the application core
pub struct CacheOperationsService<K, V>
where
    K: Debug + Hash + Eq + Send + Sync + 'static,
    V: Debug + Send + Sync + Clone + 'static,
{
    cache_manager: CacheManager<K, V>,
    _phantom: std::marker::PhantomData<(K, V)>,
}

impl<K, V> CacheOperationsService<K, V>
where
    K: Debug + Hash + Eq + Send + Sync + 'static,
    V: Debug + Send + Sync + Clone + 'static,
{
    pub fn new(cache_manager: CacheManager<K, V>) -> Self {
        Self {
            cache_manager,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Helper method to look up a cache by name
    async fn get_cache_store(&self, cache_name: &str) -> Result<Arc<dyn CacheStore<K, V>>> {
        self.cache_manager
            .get_cache(cache_name)
            .await
            .ok_or_else(|| Error::CacheNotFound(cache_name.to_string()))
    }
}

#[async_trait]
impl<K, V> CacheOperations<K, V> for CacheOperationsService<K, V>
where
    K: Debug + Hash + Eq + Send + Sync + 'static,
    V: Debug + Send + Sync + Clone + 'static,
{
    /// Execute a PUT operation on a named cache
    async fn put(
        &self,
        cache_name: &str,
        key: K,
        value: V,
        ttl: Option<TtlMs>,
    ) -> Result<PutResponse> {
        let cache_store = self.get_cache_store(cache_name).await?;
        cache_store.put(key, value, ttl).await
    }

    /// Execute a GET operation on a named cache
    async fn get(&self, cache_name: &str, key: &K) -> Result<GetResponse<V>> {
        let cache_store = self.get_cache_store(cache_name).await?;
        cache_store.get(key).await
    }

    /// Execute a DELETE operation on a named cache
    async fn delete(&self, cache_name: &str, key: &K) -> Result<DeleteResponse> {
        let cache_store = self.get_cache_store(cache_name).await?;
        cache_store.delete(key).await
    }
}

impl<K, V> std::fmt::Debug for CacheOperationsService<K, V>
where
    K: Debug + Hash + Eq + Send + Sync + 'static,
    V: Debug + Send + Sync + Clone + 'static,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CacheOperationsService")
            .field("cache_manager", &self.cache_manager)
            .finish()
    }
}
