use crate::domain::CacheConfig;
use crate::domain::response::{
    DeleteResponse, GetResponse, PutResponse,
    admin::{CreateCacheResponse, DescribeCacheResponse, DropCacheResponse, ListCachesResponse},
};
use async_trait::async_trait;
use shared::{Result, TtlMs};

// Ports are the pluggable extension points for underlying cache implementations

/// Port for cache operations (e.g., Foyer)
#[async_trait]
pub trait CacheStore<K, V>: Send + Sync + 'static {
    async fn put(&self, key: K, val: V, ttl: Option<TtlMs>) -> Result<PutResponse>;
    async fn get(&self, key: &K) -> Result<GetResponse<V>>;
    async fn delete(&self, key: &K) -> Result<DeleteResponse>;
}

/// Port for cache administration operations
#[async_trait]
pub trait AdminOperations: Send + Sync + 'static {
    async fn create_cache(&self, config: CacheConfig) -> Result<CreateCacheResponse>;
    async fn drop_cache(&self, name: &str) -> Result<DropCacheResponse>;
    async fn list_caches(&self) -> Result<ListCachesResponse>;
    async fn describe_cache(&self, name: &str) -> Result<DescribeCacheResponse>;
}
