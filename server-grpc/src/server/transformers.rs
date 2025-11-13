use crate::generated;
use carbon::domain;

/// Maps domain CacheConfig to grpc CacheConfig
pub fn domain_cache_config_to_grpc(config: domain::CacheConfig) -> generated::CacheConfig {
    generated::CacheConfig {
        name: config.name,
        mem_bytes: config.mem_bytes,
        disk_path: config.disk_path.unwrap_or_default(),
        shards: config.shards,
        policy: config.policy as i32,
        default_ttl_ms: config.default_ttl_ms.unwrap_or(0),
        max_value_bytes: config.max_value_bytes.unwrap_or(0),
    }
}

/// Maps domain CacheInfo to grpc CacheInfo
pub fn domain_cache_info_to_grpc(cache_info: domain::CacheInfo) -> generated::CacheInfo {
    generated::CacheInfo {
        config: Some(domain_cache_config_to_grpc(cache_info.config)),
        keys_estimate: cache_info.keys_estimate,
    }
}

/// Maps domain ListCachesResponse to grpc ListCachesResponse
pub fn domain_list_caches_to_grpc(
    domain_response: domain::response::admin::ListCachesResponse,
) -> generated::ListCachesResponse {
    let grpc_caches = domain_response
        .caches
        .into_iter()
        .map(domain_cache_info_to_grpc)
        .collect();

    generated::ListCachesResponse {
        caches: grpc_caches,
    }
}

/// Maps domain DescribeCacheResponse to grpc DescribeCacheResponse
pub fn domain_describe_cache_to_grpc(
    domain_response: domain::response::admin::DescribeCacheResponse,
) -> generated::DescribeCacheResponse {
    generated::DescribeCacheResponse {
        info: Some(domain_cache_info_to_grpc(domain_response.info)),
    }
}
