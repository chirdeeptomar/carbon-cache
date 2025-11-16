mod foyer_cache;
mod moka_cache;

pub use foyer_cache::FoyerMemoryCache;
pub use moka_cache::MokaCache;

use carbon::domain::CacheConfig;
use carbon::ports::{CacheStore, StorageFactory};
use std::sync::Arc;
use std::{fmt::Debug, hash::Hash};

/// Unified factory for creating cache instances from configuration
/// Supports Moka, Foyer Memory, and Foyer Hybrid backends
pub struct UnifiedStorageFactory;

impl<K, V> StorageFactory<K, V> for UnifiedStorageFactory
where
    K: Debug + Hash + Eq + Send + Sync + 'static,
    V: Debug + Send + Sync + Clone + 'static,
{
    fn create_from_config(&self, config: &CacheConfig) -> Arc<dyn CacheStore<K, V>> {
        use carbon::domain::CacheEvictionStrategy;
        use std::time::Duration;

        match config.backend {
            CacheEvictionStrategy::TimeBound => {
                // Create Moka cache with optional TTL
                let default_ttl = config
                    .default_ttl_ms
                    .map(|ms| Duration::from_millis(ms));

                // For Moka, mem_bytes could be interpreted as max entries
                // For simplicity, if mem_bytes > 0, treat as bounded with that many entries
                // Otherwise, unbounded
                let max_entries = if config.mem_bytes.is_some(){
                    config.mem_bytes
                } else {
                    None
                };

                Arc::new(MokaCache::new(
                    config.name.clone(),
                    max_entries,
                    default_ttl,
                ))
            }
            CacheEvictionStrategy::SizeBounded => {
                // Create Foyer in-memory cache
                // Safety: mem_bytes is validated as required for SizeBounded caches
                Arc::new(FoyerMemoryCache::new(
                    config.name.clone(),
                    config.mem_bytes.expect("mem_bytes is required for SizeBounded cache and should be validated") as usize,
                ))
            }
            CacheEvictionStrategy::OverflowToDisk => {
                // TODO: Implement Foyer hybrid (memory + disk)
                // For now, fallback to memory-only
                // Safety: mem_bytes is validated as required for OverflowToDisk caches
                Arc::new(FoyerMemoryCache::new(
                    config.name.clone(),
                    config.mem_bytes.expect("mem_bytes is required for OverflowToDisk cache and should be validated") as usize,
                ))
            }
        }
    }
}

/// Legacy factory for backward compatibility
/// Deprecated: Use UnifiedStorageFactory instead
#[deprecated(note = "Use UnifiedStorageFactory instead")]
pub struct FoyerStorageFactory;

#[allow(deprecated)]
impl<K, V> StorageFactory<K, V> for FoyerStorageFactory
where
    K: Debug + Hash + Eq + Send + Sync + 'static,
    V: Debug + Send + Sync + Clone + 'static,
{
    fn create_from_config(&self, config: &CacheConfig) -> Arc<dyn CacheStore<K, V>> {
        // Delegate to UnifiedStorageFactory
        UnifiedStorageFactory.create_from_config(config)
    }
}
