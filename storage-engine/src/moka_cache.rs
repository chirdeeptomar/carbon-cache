use async_trait::async_trait;
use carbon::domain::response::{DeleteResponse, GetResponse, PutResponse};
use carbon::ports::CacheStore;
use moka::future::Cache;
use shared::{Error, Result, TtlMs};
use std::fmt::Debug;
use std::hash::Hash;
use std::time::Duration;

/// Moka-based cache implementation with TTL support
/// Provides lock-free, concurrent cache with optional size bounds and TTL
pub struct MokaCache<K, V>
where
    K: Debug + Hash + Eq + Send + Sync + 'static,
    V: Debug + Send + Sync + Clone + 'static,
{
    cache: Cache<K, V>,
}

impl<K, V> MokaCache<K, V>
where
    K: Debug + Hash + Eq + Send + Sync + 'static,
    V: Debug + Send + Sync + Clone + 'static,
{
    /// Create a new unbounded Moka cache with optional default TTL
    pub fn new_unbounded(default_ttl: Option<Duration>) -> Self {
        let mut builder = Cache::builder();

        if let Some(ttl) = default_ttl {
            builder = builder.time_to_live(ttl);
        }

        Self {
            cache: builder.build(),
        }
    }

    /// Create a new bounded Moka cache with max entries and optional default TTL
    pub fn new_bounded(max_entries: u64, default_ttl: Option<Duration>) -> Self {
        let mut builder = Cache::builder().max_capacity(max_entries);

        if let Some(ttl) = default_ttl {
            builder = builder.time_to_live(ttl);
        }

        Self {
            cache: builder.build(),
        }
    }

    /// Create a Moka cache from name and optional capacity
    /// Used for compatibility with factory pattern
    pub fn new(name: String, max_entries: Option<u64>, default_ttl: Option<Duration>) -> Self {
        let mut builder = Cache::builder().name(&name);

        if let Some(capacity) = max_entries {
            builder = builder.max_capacity(capacity);
        }

        if let Some(ttl) = default_ttl {
            builder = builder.time_to_live(ttl);
        }

        Self {
            cache: builder.build(),
        }
    }
}

#[async_trait]
impl<K, V> CacheStore<K, V> for MokaCache<K, V>
where
    K: Debug + Hash + Eq + Send + Sync + 'static,
    V: Debug + Send + Sync + Clone + 'static,
{
    async fn put(&self, key: K, val: V, ttl: Option<TtlMs>) -> Result<PutResponse> {
        // Note: Moka uses global TTL configured at cache creation time
        // Per-entry TTL is not directly supported in current moka version
        // If per-entry TTL is requested, we use the global TTL instead
        if ttl.is_some() {
            // Log that per-entry TTL is being ignored (would need logging setup)
            // For now, just use the global TTL configured in the cache builder
        }

        self.cache.insert(key, val).await;
        Ok(PutResponse::new(true, "Successfully inserted"))
    }

    async fn get(&self, key: &K) -> Result<GetResponse<V>> {
        match self.cache.get(key).await {
            Some(value) => Ok(GetResponse::new(true, value)),
            None => Err(Error::NotFound), // Either doesn't exist or TTL expired
        }
    }

    async fn delete(&self, key: &K) -> Result<DeleteResponse> {
        let existed = self.cache.remove(key).await.is_some();
        Ok(DeleteResponse::new(existed))
    }
}

impl<K, V> Debug for MokaCache<K, V>
where
    K: Debug + Hash + Eq + Send + Sync + 'static,
    V: Debug + Send + Sync + Clone + 'static,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MokaCache")
            .field("entry_count", &self.cache.entry_count())
            .field("weighted_size", &self.cache.weighted_size())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_moka_cache_put_and_get() {
        let cache = MokaCache::new("test".to_string(), None, None);

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
    async fn test_moka_cache_delete() {
        let cache = MokaCache::new("test".to_string(), None, None);

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
    async fn test_moka_cache_get_nonexistent() {
        let cache: MokaCache<&str, &str> = MokaCache::new("test".to_string(), None, None);

        // Try to get a key that doesn't exist
        let result = cache.get(&"nonexistent").await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::NotFound));
    }

    #[tokio::test]
    async fn test_moka_cache_overwrite() {
        let cache = MokaCache::new("test".to_string(), None, None);

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
    async fn test_moka_cache_with_per_entry_ttl() {
        // Note: Moka doesn't support per-entry TTL in current version
        // This test verifies that per-entry TTL parameter is accepted but uses global TTL
        let cache = MokaCache::new("test".to_string(), None, None);

        let key = "ttl_key";
        let value = "ttl_value";

        // Put with TTL parameter (will be ignored, uses global TTL)
        cache
            .put(key, value, Some(TtlMs(100)))
            .await
            .unwrap();

        // Should be available since no global TTL is set
        let get_response = cache.get(&key).await.unwrap();
        assert_eq!(get_response.message, value);
    }

    #[tokio::test]
    async fn test_moka_cache_with_global_ttl() {
        let cache = MokaCache::new(
            "test".to_string(),
            None,
            Some(Duration::from_millis(100)),
        );

        let key = "global_ttl_key";
        let value = "global_ttl_value";

        // Put without specific TTL (uses global)
        cache.put(key, value, None).await.unwrap();

        // Should be available immediately
        let get_response = cache.get(&key).await.unwrap();
        assert_eq!(get_response.message, value);

        // Wait for expiration
        sleep(Duration::from_millis(150)).await;

        // Should be expired now
        let result = cache.get(&key).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::NotFound));
    }

    #[tokio::test]
    async fn test_moka_cache_bounded() {
        let cache = MokaCache::new_bounded(2, None); // Max 2 entries

        // Insert 3 entries
        cache.put("key1", "value1", None).await.unwrap();
        cache.put("key2", "value2", None).await.unwrap();
        cache.put("key3", "value3", None).await.unwrap();

        // Wait for eviction to take effect
        sleep(Duration::from_millis(50)).await;

        // One of the first two should be evicted
        let entry_count = cache.cache.entry_count();
        assert!(entry_count <= 2, "Cache should have at most 2 entries");
    }
}
