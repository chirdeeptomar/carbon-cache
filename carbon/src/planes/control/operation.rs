use async_trait::async_trait;

use shared::Result;

use crate::domain::{
    CacheConfig,
    response::admin::{
        CreateCacheResponse, DescribeCacheResponse, DropCacheResponse, ListCachesResponse,
    },
};

#[async_trait]
pub trait AdminOperations {
    async fn create_cache(&self, config: CacheConfig) -> Result<CreateCacheResponse>;
    async fn drop_cache(&self, name: &str) -> Result<DropCacheResponse>;
    async fn list_caches(&self) -> Result<ListCachesResponse>;
    async fn describe_cache(&self, name: &str) -> Result<DescribeCacheResponse>;
}
