use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CacheItemEvent {
    Added(ItemAddedEvent),
    Updated(ItemUpdatedEvent),
    Deleted(ItemDeletedEvent),
}

impl CacheItemEvent {
    pub fn cache_name(&self) -> &str {
        match self {
            CacheItemEvent::Added(e) => &e.cache_name,
            CacheItemEvent::Updated(e) => &e.cache_name,
            CacheItemEvent::Deleted(e) => &e.cache_name,
        }
    }

    pub fn key(&self) -> &[u8] {
        match self {
            CacheItemEvent::Added(e) => &e.key,
            CacheItemEvent::Updated(e) => &e.key,
            CacheItemEvent::Deleted(e) => &e.key,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemAddedEvent {
    pub cache_name: String,
    #[serde(with = "serde_bytes")]
    pub key: Vec<u8>,
    pub value_size: usize,
    pub ttl_ms: Option<u64>,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemUpdatedEvent {
    pub cache_name: String,
    #[serde(with = "serde_bytes")]
    pub key: Vec<u8>,
    pub value_size: usize,
    pub ttl_ms: Option<u64>,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemDeletedEvent {
    pub cache_name: String,
    #[serde(with = "serde_bytes")]
    pub key: Vec<u8>,
    pub timestamp: u64,
}

/// Helper to get current timestamp in seconds since UNIX epoch
pub fn now_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
