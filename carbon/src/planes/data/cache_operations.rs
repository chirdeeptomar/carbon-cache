use crate::domain::response::{DeleteResponse, GetResponse, PutResponse};
use crate::events::{now_timestamp, CacheItemEvent, ItemAddedEvent, ItemDeletedEvent, ItemUpdatedEvent};
use crate::planes::control::CacheManager;
use crate::planes::data::operation::CacheOperations;
use crate::ports::CacheStore;
use async_trait::async_trait;
use shared::{Error, Result, TtlMs};
use std::fmt::Debug;
use std::hash::Hash;
use std::sync::Arc;
use tokio::sync::broadcast;

/// Application service that orchestrates cache operations
/// This is the main entry point for all cache operations in the application core
#[derive(Clone)]
pub struct CacheOperationsService<K, V>
where
    K: Debug + Hash + Eq + Send + Sync + Clone + 'static,
    V: Debug + Send + Sync + Clone + 'static,
{
    cache_manager: CacheManager<K, V>,
    event_broadcaster: Option<broadcast::Sender<CacheItemEvent>>,
}

impl<K, V> CacheOperationsService<K, V>
where
    K: Debug + Hash + Eq + Send + Sync + Clone + 'static,
    V: Debug + Send + Sync + Clone + 'static,
{
    pub fn new(cache_manager: CacheManager<K, V>) -> Self {
        Self {
            cache_manager,
            event_broadcaster: None,
        }
    }

    pub fn with_event_broadcaster(
        cache_manager: CacheManager<K, V>,
        broadcaster: broadcast::Sender<CacheItemEvent>,
    ) -> Self {
        Self {
            cache_manager,
            event_broadcaster: Some(broadcaster),
        }
    }

    /// Helper method to look up a cache by name
    async fn get_cache_store(&self, cache_name: &str) -> Result<Arc<dyn CacheStore<K, V>>> {
        self.cache_manager
            .get_cache_store(cache_name)
            .await
            .ok_or_else(|| Error::CacheNotFound(cache_name.to_string()))
    }
}

// Generic implementation removed - using specialized implementation below for String/Vec<u8>
impl<K, V> std::fmt::Debug for CacheOperationsService<K, V>
where
    K: Debug + Hash + Eq + Send + Sync + Clone + 'static,
    V: Debug + Send + Sync + Clone + 'static,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CacheOperationsService")
            .field("cache_manager", &self.cache_manager)
            .finish()
    }
}

// Specialized implementation for Vec<u8>/Vec<u8> with event broadcasting
#[async_trait]
impl CacheOperations<Vec<u8>, Vec<u8>> for CacheOperationsService<Vec<u8>, Vec<u8>> {
    /// Execute a PUT operation on a named cache with event broadcasting
    async fn put(
        &self,
        cache_name: &str,
        key: Vec<u8>,
        value: Vec<u8>,
        ttl: Option<TtlMs>,
    ) -> Result<PutResponse> {
        let cache_store = self.get_cache_store(cache_name).await?;

        // Check if key exists to determine if this is an add or update
        let existed = cache_store.get(&key).await.is_ok();

        // Perform the put operation
        let result = cache_store.put(key.clone(), value.clone(), ttl).await?;

        // Broadcast event if broadcaster is configured
        if let Some(ref broadcaster) = self.event_broadcaster {
            let event_type = if existed { "updated" } else { "added" };
            let event = if existed {
                CacheItemEvent::Updated(ItemUpdatedEvent {
                    cache_name: cache_name.to_string(),
                    key: key.clone(),
                    value_size: value.len(),
                    ttl_ms: ttl.map(|t| t.0),
                    timestamp: now_timestamp(),
                })
            } else {
                CacheItemEvent::Added(ItemAddedEvent {
                    cache_name: cache_name.to_string(),
                    key: key.clone(),
                    value_size: value.len(),
                    ttl_ms: ttl.map(|t| t.0),
                    timestamp: now_timestamp(),
                })
            };

            match broadcaster.send(event) {
                Ok(subscriber_count) => {
                    tracing::debug!(
                        "Broadcasted {} event for key '{:?}' in cache '{}' to {} subscriber(s)",
                        event_type,
                        String::from_utf8_lossy(&key),
                        cache_name,
                        subscriber_count
                    );
                }
                Err(_) => {
                    tracing::warn!(
                        "No subscribers for {} event on key '{:?}' in cache '{}'",
                        event_type,
                        String::from_utf8_lossy(&key),
                        cache_name
                    );
                }
            }
        }

        Ok(result)
    }

    /// Execute a GET operation on a named cache (no event broadcasting)
    async fn get(&self, cache_name: &str, key: &Vec<u8>) -> Result<GetResponse<Vec<u8>>> {
        let cache_store = self.get_cache_store(cache_name).await?;
        cache_store.get(key).await
    }

    /// Execute a DELETE operation on a named cache with event broadcasting
    async fn delete(&self, cache_name: &str, key: &Vec<u8>) -> Result<DeleteResponse> {
        let cache_store = self.get_cache_store(cache_name).await?;
        let result = cache_store.delete(key).await?;

        // Broadcast event only if key actually existed (was deleted)
        if result.deleted {
            if let Some(ref broadcaster) = self.event_broadcaster {
                let event = CacheItemEvent::Deleted(ItemDeletedEvent {
                    cache_name: cache_name.to_string(),
                    key: key.clone(),
                    timestamp: now_timestamp(),
                });

                match broadcaster.send(event) {
                    Ok(subscriber_count) => {
                        tracing::debug!(
                            "Broadcasted deleted event for key '{:?}' in cache '{}' to {} subscriber(s)",
                            String::from_utf8_lossy(key),
                            cache_name,
                            subscriber_count
                        );
                    }
                    Err(_) => {
                        tracing::warn!(
                            "No subscribers for deleted event on key '{:?}' in cache '{}'",
                            String::from_utf8_lossy(key),
                            cache_name
                        );
                    }
                }
            }
        }

        Ok(result)
    }
}
