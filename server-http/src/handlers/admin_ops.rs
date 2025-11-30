use crate::models::{
    CreateCacheRequest, CreateCacheResponse, DropCacheResponse, ValidationErrorResponse,
};
use crate::state::AppState;
use crate::validation::CacheConfigFactory;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use carbon::ports::{AdminOperations, StorageFactory};
use storage_engine::UnifiedStorageFactory;
use tracing::info;

/// POST /admin/caches
pub async fn create_cache(
    State(state): State<AppState>,
    Json(req): Json<CreateCacheRequest>,
) -> Result<Json<CreateCacheResponse>, (StatusCode, Json<ValidationErrorResponse>)> {
    info!("CREATE_CACHE: name={}, backend={}", req.name, req.eviction);

    // Validate and build config using factory
    let config = match CacheConfigFactory::from_request(req) {
        Ok(config) => config,
        Err(err) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ValidationErrorResponse {
                    error: err.to_string(),
                    field: None,
                    details: Some(format!("{:?}", err)),
                }),
            ))
        }
    };

    // Use factory to create appropriate storage backend
    let factory = UnifiedStorageFactory;
    let storage = factory.create_from_config(&config);

    // Create cache with storage (unified operation)
    match state.cache_manager.create_cache(config, storage).await {
        Ok(result) => Ok(Json(CreateCacheResponse {
            created: result.created,
            message: result.message,
        })),
        Err(_) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ValidationErrorResponse {
                error: "Failed to create cache".to_string(),
                field: None,
                details: Some("Internal server error".to_string()),
            }),
        )),
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
            let json =
                serde_json::to_value(result).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
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
            let json =
                serde_json::to_value(result).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            Ok(Json(json))
        }
        Err(shared::Error::CacheNotFound(_)) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}
