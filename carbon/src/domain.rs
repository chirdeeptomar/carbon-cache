pub mod request {}

pub mod response {

    pub mod admin {
        use crate::domain::CacheInfo;

        #[derive(Clone, Debug)]
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

        #[derive(Clone, Debug)]
        pub struct DropCacheResponse {
            pub dropped: bool,
        }

        impl DropCacheResponse {
            pub fn new(dropped: bool) -> Self {
                Self { dropped }
            }
        }

        #[derive(Clone, Debug)]
        pub struct ListCachesResponse {
            pub caches: Vec<CacheInfo>,
        }

        impl ListCachesResponse {
            pub fn new(caches: Vec<CacheInfo>) -> Self {
                Self { caches }
            }
        }

        #[derive(Clone, Debug)]
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

#[derive(Clone, Debug)]
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

#[derive(Clone, Debug)]
pub struct CacheConfig {
    pub name: String,                 // unique cache name
    pub mem_bytes: u64,               // RAM budget
    pub disk_path: Option<String>,    // NVMe dir (optional -> memory-only)
    pub shards: u32,                  // default: 2 * cores
    pub policy: EvictionPolicy,       // default: TINYLFU
    pub default_ttl_ms: Option<u64>,  // 0 = no default TTL
    pub max_value_bytes: Option<u64>, // guardrails
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
    ) -> Self {
        Self {
            name: name.into(),
            mem_bytes,
            disk_path,
            shards,
            policy,
            default_ttl_ms,
            max_value_bytes,
        }
    }
}

#[repr(i8)]
#[derive(Clone, Copy, Debug)]
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
