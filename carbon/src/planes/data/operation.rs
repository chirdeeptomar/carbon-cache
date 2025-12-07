use crate::domain::response::{DeleteResponse, GetResponse, PutResponse};
use async_trait::async_trait;
use shared::Result;

/// Application-level cache operations trait
/// This is for orchestrating operations across named caches
#[async_trait]
pub trait CacheOperations<K, V>: Send + Sync + 'static {
    async fn put(&self, cache_name: &str, key: K, value: V) -> Result<PutResponse>;

    async fn get(&self, cache_name: &str, key: &K) -> Result<GetResponse<V>>;

    async fn delete(&self, cache_name: &str, key: &K) -> Result<DeleteResponse>;
}
