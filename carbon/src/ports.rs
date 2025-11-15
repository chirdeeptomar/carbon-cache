use std::sync::Arc;
use crate::domain::response::{
    DeleteResponse, GetResponse, PutResponse,
    admin::{DescribeCacheResponse, DropCacheResponse, ListCachesResponse},
};
use async_trait::async_trait;
use shared::{Result, TtlMs};
use crate::domain::CacheConfig;
use crate::domain::response::admin::CreateCacheResponse;
// Ports are the pluggable extension points for underlying cache implementations

/// Port for cache operations (e.g., Foyer)
#[async_trait]
pub trait CacheStore<K, V>: Send + Sync + 'static {
    async fn put(&self, key: K, val: V, ttl: Option<TtlMs>) -> Result<PutResponse>;
    async fn get(&self, key: &K) -> Result<GetResponse<V>>;
    async fn delete(&self, key: &K) -> Result<DeleteResponse>;
}

/// Port for creating cache storage from configuration
/// This allows different storage backends to be plugged in
pub trait StorageFactory<K, V>: Send + Sync + 'static {
    /// Create a new cache store from configuration
    fn create_from_config(&self, config: &CacheConfig) -> Arc<dyn CacheStore<K, V>>;
}

/// Port for cache administration operations
#[async_trait]
pub trait AdminOperations<K,V>: Send + Sync + 'static {
    async fn create_cache(
        &self,
        config: CacheConfig,
        store: Arc<dyn CacheStore<K, V>>,
    ) -> Result<CreateCacheResponse>;
    async fn drop_cache(&self, name: &str) -> Result<DropCacheResponse>;
    async fn list_caches(&self) -> Result<ListCachesResponse>;
    async fn describe_cache(&self, name: &str) -> Result<DescribeCacheResponse>;
}
