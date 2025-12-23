#![deny(clippy::all)]

use crate::domain::CacheConfig;
use crate::domain::response::ExistsResponse;
use crate::domain::response::{DeleteResponse, GetResponse, PutResponse};
use async_trait::async_trait;
use shared::Result;
use std::sync::Arc;

// Ports are the pluggable extension points for underlying cache implementations

/// Port for creating cache storage from configuration
/// This allows different storage backends to be plugged in
pub trait StorageFactory<K, V>: Send + Sync + 'static {
    /// Create a new cache store from configuration
    fn create_from_config(&self, config: &CacheConfig) -> Arc<dyn CacheStore<K, V>>;
}

/// Port for cache operations (e.g., Foyer)
#[async_trait]
pub trait CacheStore<K, V>: Send + Sync + 'static {
    async fn exists(&self, key: &K) -> Result<ExistsResponse>;
    async fn put(&self, key: K, val: V) -> Result<PutResponse>;
    async fn get(&self, key: &K) -> Result<GetResponse<V>>;
    async fn delete(&self, key: &K) -> Result<DeleteResponse>;
}
