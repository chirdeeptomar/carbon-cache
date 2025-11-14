use std::collections::HashMap;

pub mod request {}

pub mod response {

    pub mod admin {
        use crate::domain::CacheInfo;
        use serde::Serialize;

        #[derive(Clone, Debug, Serialize)]
        pub struct CreateCacheResponse {
            pub created: bool,
            pub message: String,
        }

        impl CreateCacheResponse {
            pub fn new(created: bool, message: impl Into<String>) -> Self {
                Self {
                    created,
                    message: message.into(),
                }
            }
        }

        #[derive(Clone, Debug, Serialize)]
        pub struct DropCacheResponse {
            pub dropped: bool,
        }

        impl DropCacheResponse {
            pub fn new(dropped: bool) -> Self {
                Self { dropped }
            }
        }

        #[derive(Clone, Debug, Serialize)]
        pub struct ListCachesResponse {
            pub caches: Vec<CacheInfo>,
        }

        impl ListCachesResponse {
            pub fn new(caches: Vec<CacheInfo>) -> Self {
                Self { caches }
            }
        }

        #[derive(Clone, Debug, Serialize)]
        pub struct DescribeCacheResponse {
            pub info: CacheInfo,
        }

        impl DescribeCacheResponse {
            pub fn new(info: CacheInfo) -> Self {
                Self { info }
            }
        }
    }

    #[derive(Clone, Debug)]
    pub struct PutResponse {
        pub created: bool,
        pub message: String,
    }

    impl PutResponse {
        pub fn new(created: bool, message: impl Into<String>) -> Self {
            Self {
                created,
                message: message.into(),
            }
        }
    }

    #[derive(Clone, Debug)]
    pub struct GetResponse<V> {
        pub found: bool,
        pub message: V,
    }

    impl<V> GetResponse<V> {
        pub fn new(found: bool, message: V) -> Self {
            Self { found, message }
        }
    }

    #[derive(Clone, Debug)]
    pub struct DeleteResponse {
        pub deleted: bool,
    }

    impl DeleteResponse {
        pub fn new(deleted: bool) -> Self {
            Self { deleted }
        }
    }
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct CacheInfo {
    pub config: CacheConfig,
    pub keys_estimate: u64,
    pub size_estimate: u64,
}

impl CacheInfo {
    pub fn from_config(config: CacheConfig) -> Self {
        Self {
            config,
            keys_estimate: 0,
            size_estimate: 0,
        }
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct CacheConfig {
    pub name: String,                 // unique cache name
    pub mem_bytes: u64,               // RAM budget
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disk_path: Option<String>,    // NVMe dir (optional -> memory-only)
    pub shards: u32,                  // default: 2 * cores
    pub policy: EvictionPolicy,       // default: TINYLFU
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_ttl_ms: Option<u64>,  // 0 = no default TTL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_value_bytes: Option<u64>, // guardrails
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,  // human-readable description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<HashMap<String, String>>, // metadata tags for categorization
}

impl CacheConfig {
    pub fn new(
        name: impl Into<String>,
        mem_bytes: u64,
        disk_path: Option<String>,
        shards: u32,
        policy: EvictionPolicy,
        default_ttl_ms: Option<u64>,
        max_value_bytes: Option<u64>,
        description: Option<String>,
        tags: Option<HashMap<String, String>>,
    ) -> Self {
        Self {
            name: name.into(),
            mem_bytes,
            disk_path,
            shards,
            policy,
            default_ttl_ms,
            max_value_bytes,
            description,
            tags,
        }
    }

    /// Builder method to add description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Builder method to add tags
    pub fn with_tags(mut self, tags: HashMap<String, String>) -> Self {
        self.tags = Some(tags);
        self
    }
}

#[repr(i8)]
#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub enum EvictionPolicy {
    Unspecified,
    Lru,
    TinyLfu,
    Sieve,
}

impl TryFrom<i32> for EvictionPolicy {
    type Error = &'static str;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(EvictionPolicy::Unspecified),
            1 => Ok(EvictionPolicy::Lru),
            2 => Ok(EvictionPolicy::TinyLfu),
            3 => Ok(EvictionPolicy::Sieve),
            _ => Err("Invalid eviction policy value"),
        }
    }
}
