use std::sync::Arc;

use async_trait::async_trait;

use shared::Result;

use crate::{
    domain::{
        CacheConfig,
        response::admin::{
            CreateCacheResponse, DescribeCacheResponse, DropCacheResponse, ListCachesResponse,
        },
    },
    ports::CacheStore,
};

#[async_trait]
pub trait AdminOperations<K, V>: Send + Sync + 'static {
    async fn create_cache(
        &self,
        config: CacheConfig,
        store: Arc<dyn CacheStore<K, V>>,
    ) -> Result<CreateCacheResponse>;
    async fn drop_cache(&self, name: &str) -> Result<DropCacheResponse>;
    async fn list_caches(&self) -> Result<ListCachesResponse>;
    async fn describe_cache(&self, name: &str) -> Result<DescribeCacheResponse>;
}
