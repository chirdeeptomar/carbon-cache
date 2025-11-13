use async_trait::async_trait;
use carbon::domain::response::{DeleteResponse, GetResponse, PutResponse};
use carbon::ports::CacheStore;
use foyer::{Cache, CacheBuilder};
use shared::{Error, Result, TtlMs};
use std::sync::Arc;
use std::{fmt::Debug, hash::Hash};

/// Foyer-based in-memory cache implementation
pub struct FoyerCache<K, V>
where
    K: Debug + Hash + Eq + Send + Sync + 'static,
    V: Debug + Send + Sync + Clone + 'static,
{
    cache: Arc<Cache<K, V>>,
}

impl<K, V> FoyerCache<K, V>
where
    K: Debug + Hash + Eq + Send + Sync + 'static,
    V: Debug + Send + Sync + Clone + 'static,
{
    /// Create a new Foyer in-memory cache with the given memory capacity in bytes
    pub fn new(name: String, mem_bytes: usize) -> Self {
        let cache = CacheBuilder::new(mem_bytes).with_name(name).build();

        Self {
            cache: Arc::new(cache),
        }
    }

    /// Create a new FoyerCache with custom configuration
    /// Note: disk_path parameter is accepted but not used yet (Foyer hybrid cache requires more setup)
    pub fn with_config(mem_bytes: usize, _disk_path: Option<String>) -> Self {
        // TODO: Implement hybrid cache with disk persistence
        // For now, just create an in-memory cache
        let cache = CacheBuilder::new(mem_bytes).build();

        Self {
            cache: Arc::new(cache),
        }
    }
}

#[async_trait]
impl<K, V> CacheStore<K, V> for FoyerCache<K, V>
where
    K: Debug + Hash + Eq + Send + Sync + 'static,
    V: Debug + Send + Sync + Clone + 'static,
{
    async fn put(&self, key: K, val: V, _ttl: Option<TtlMs>) -> Result<PutResponse> {
        // TODO: Foyer doesn't have built-in per-entry TTL in the basic API
        // Would need to implement a separate TTL tracking mechanism with background cleanup
        if _ttl.is_some() {
            // For now, just log that TTL is requested but not implemented
            // In production, you'd want to either:
            // 1. Use a wrapper that tracks expiry times
            // 2. Use Foyer's hybrid cache with custom eviction
            // 3. Implement a background task to clean up expired entries
        }

        self.cache.insert(key, val);
        Ok(PutResponse::new(true, "Successfully inserted"))
    }

    async fn get(&self, key: &K) -> Result<GetResponse<V>> {
        match self.cache.get(key) {
            Some(entry) => {
                let value = entry.value();
                Ok(GetResponse::new(true, value.clone()))
            }
            None => Err(Error::NotFound),
        }
    }

    async fn delete(&self, key: &K) -> Result<DeleteResponse> {
        let existed = self.cache.remove(key).is_some();
        Ok(DeleteResponse::new(existed))
    }
}

impl<K, V> Debug for FoyerCache<K, V>
where
    K: Debug + Hash + Eq + Send + Sync + 'static,
    V: Debug + Send + Sync + Clone + 'static,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FoyerCache")
            .field("cache", &"<foyer::Cache>")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_foyer_cache_put_and_get() {
        let cache = FoyerCache::new("test".to_string(),1024 * 1024); // 1MB

        // Put a value
        let key = "hello";
        let value = "world";
        let put_response = cache.put(key, value, None).await.unwrap();
        assert!(put_response.created);
        assert_eq!(put_response.message, "Successfully inserted");

        // Get the value
        let get_response = cache.get(&key).await.unwrap();
        assert!(get_response.found);
        assert_eq!(get_response.message, value);
    }

    #[tokio::test]
    async fn test_foyer_cache_delete() {
        let cache = FoyerCache::new("test".to_string(), 1024 * 1024);

        let key = "test_key";
        let value = "test_value";

        // Put a value
        cache.put(key, value, None).await.unwrap();

        // Delete the value
        let delete_response = cache.delete(&key).await.unwrap();
        assert!(delete_response.deleted);

        // Try to get deleted value
        let result = cache.get(&key).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::NotFound));
    }

    #[tokio::test]
    async fn test_foyer_cache_get_nonexistent() {
        let cache: FoyerCache<&str, &str> = FoyerCache::new("test".to_string(),1024 * 1024);

        // Try to get a key that doesn't exist
        let result = cache.get(&"nonexistent").await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::NotFound));
    }

    #[tokio::test]
    async fn test_foyer_cache_overwrite() {
        let cache = FoyerCache::new("test".to_string(), 1024 * 1024);

        let key = "key";

        // Put initial value
        cache.put(key, "value1", None).await.unwrap();

        // Overwrite with new value
        cache.put(key, "value2", None).await.unwrap();

        // Get the value - should be the new one
        let get_response = cache.get(&key).await.unwrap();
        assert_eq!(get_response.message, "value2");
    }

    #[tokio::test]
    async fn test_foyer_cache_with_ttl() {
        let cache = FoyerCache::new("test".to_string(),1024 * 1024);

        let key = "ttl_key";
        let value = "ttl_value";

        // Put with TTL (note: currently not enforced, just accepted)
        let put_response = cache.put(key, value, Some(TtlMs(5000))).await.unwrap();
        assert!(put_response.created);

        // Should still be able to get it immediately
        let get_response = cache.get(&key).await.unwrap();
        assert_eq!(get_response.message, value);
    }
}
