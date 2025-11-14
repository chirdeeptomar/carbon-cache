use crate::models::{CreateCacheRequest, CreateCacheResponse, DropCacheResponse};
use crate::state::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use carbon::domain::{CacheConfig, EvictionPolicy};
use carbon::ports::AdminOperations;
use std::sync::Arc;
use storage_engine::FoyerCache;
use tracing::info;

/// POST /admin/caches
pub async fn create_cache(
    State(state): State<AppState>,
    Json(req): Json<CreateCacheRequest>,
) -> Result<Json<CreateCacheResponse>, StatusCode> {
    info!("CREATE_CACHE: name={}", req.name);

    let policy = match req.policy.to_lowercase().as_str() {
        "lru" => EvictionPolicy::Lru,
        "sieve" => EvictionPolicy::Sieve,
        "tinylfu" | "" => EvictionPolicy::TinyLfu,
        _ => EvictionPolicy::TinyLfu,
    };

    let config = CacheConfig::new(
        req.name.clone(),
        req.mem_bytes,
        req.disk_path.clone(),
        req.shards,
        policy,
        if req.default_ttl_ms > 0 {
            Some(req.default_ttl_ms)
        } else {
            None
        },
        if req.max_value_bytes > 0 {
            Some(req.max_value_bytes)
        } else {
            None
        },
        req.description.clone(),
        req.tags.clone(),
    );

    // Instantiate the storage layer
    let foyer_cache = FoyerCache::new(req.name.clone(), req.mem_bytes as usize);

    // Create cache with storage (unified operation)
    match state
        .cache_manager
        .create_cache(config, Arc::new(foyer_cache))
        .await
    {
        Ok(result) => Ok(Json(CreateCacheResponse {
            created: result.created,
            message: result.message,
        })),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// DELETE /admin/caches/:name
pub async fn drop_cache(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<DropCacheResponse>, StatusCode> {
    info!("DROP_CACHE: name={}", name);

    match state.cache_manager.drop_cache(&name).await {
        Ok(result) => Ok(Json(DropCacheResponse {
            dropped: result.dropped,
        })),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// GET /admin/caches
pub async fn list_caches(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    info!("LIST_CACHES");

    match state.cache_manager.list_caches().await {
        Ok(result) => {
            // Convert to JSON
            let json = serde_json::to_value(result)
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            Ok(Json(json))
        }
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// GET /admin/caches/:name
pub async fn describe_cache(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    info!("DESCRIBE_CACHE: name={}", name);

    match state.cache_manager.describe_cache(&name).await {
        Ok(result) => {
            let json = serde_json::to_value(result)
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            Ok(Json(json))
        }
        Err(shared::Error::CacheNotFound(_)) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}
