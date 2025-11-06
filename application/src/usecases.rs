use crate::ports::CacheStore;
use shared::{Key, Result, TtlMs, Value};
use std::sync::Arc;

#[derive(Clone)]
pub struct PutUseCase {
    store: Arc<dyn CacheStore>,
}
impl PutUseCase {
    pub fn new(store: Arc<dyn CacheStore>) -> Self {
        Self { store }
    }
    pub async fn exec(&self, key: Key, val: Value, ttl: Option<TtlMs>) -> Result<()> {
        // business rules, validation, metrics hooks, etc.
        self.store.put(key, val, ttl).await
    }
}

#[derive(Clone)]
pub struct GetUseCase {
    store: Arc<dyn CacheStore>,
}
impl GetUseCase {
    pub fn new(store: Arc<dyn CacheStore>) -> Self {
        Self { store }
    }
    pub async fn exec(&self, key: Key) -> Result<Value> {
        self.store.get(&key).await
    }
}

#[derive(Clone)]
pub struct DeleteUseCase {
    store: Arc<dyn CacheStore>,
}
impl DeleteUseCase {
    pub fn new(store: Arc<dyn CacheStore>) -> Self {
        Self { store }
    }
    pub async fn exec(&self, key: Key) -> Result<bool> {
        self.store.delete(&key).await
    }
}
