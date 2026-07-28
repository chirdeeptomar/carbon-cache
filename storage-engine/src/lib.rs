mod foyer_cache;
mod moka_cache;

pub use foyer_cache::FoyerMemoryCache;
pub use moka_cache::MokaCache;

use carbon::domain::CacheConfig;
use carbon::ports::{CacheStore, StorageFactory};
use shared::{Error, Result};
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
    fn create_from_config(&self, config: &CacheConfig) -> Result<Arc<dyn CacheStore<K, V>>> {
        use carbon::domain::CacheEvictionStrategy;
        use std::time::Duration;

        match config.backend {
            CacheEvictionStrategy::TimeBound => {
                // Create Moka cache with optional TTL
                let default_ttl = config.default_ttl_ms.map(Duration::from_millis);

                // For Moka, mem_bytes could be interpreted as max entries
                // For simplicity, if mem_bytes > 0, treat as bounded with that many entries
                // Otherwise, unbounded
                let max_entries = if config.mem_bytes.is_some() {
                    config.mem_bytes
                } else {
                    None
                };

                Ok(Arc::new(MokaCache::new(
                    config.name.clone(),
                    max_entries,
                    default_ttl,
                )))
            }

            CacheEvictionStrategy::SizeBounded => {
                // Create Foyer in-memory cache
                let mem_bytes = config.mem_bytes.ok_or_else(|| {
                    Error::InvalidArgument("mem_bytes is required for SizeBounded cache".into())
                })?;
                Ok(Arc::new(FoyerMemoryCache::new(
                    config.name.clone(),
                    mem_bytes as usize,
                )))
            }

            CacheEvictionStrategy::OverflowToDisk => {
                // TODO: Implement Foyer hybrid (memory + disk)
                // For now, fallback to memory-only
                let mem_bytes = config.mem_bytes.ok_or_else(|| {
                    Error::InvalidArgument("mem_bytes is required for OverflowToDisk cache".into())
                })?;
                Ok(Arc::new(FoyerMemoryCache::new(
                    config.name.clone(),
                    mem_bytes as usize,
                )))
            }
        }
    }
}
