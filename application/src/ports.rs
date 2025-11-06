// application/src/ports.rs
use async_trait::async_trait;
use shared::{Key, Result, TtlMs, Value};

#[async_trait]
pub trait CacheStore: Send + Sync + 'static {
    async fn put(&self, key: Key, val: Value, ttl: Option<TtlMs>) -> Result<()>;
    async fn get(&self, key: &Key) -> Result<Value>;
    async fn delete(&self, key: &Key) -> Result<bool>;
}
