use shared_http::api::requests::CreateCacheRequest;

use crate::leader_proxy::forward_to_leader;
use crate::state::AppState;
use crate::validation::CacheConfigFactory;
use axum::{
    body::Body,
    extract::{Path, State},
    http::{Request, StatusCode},
    Json,
};
use shared::Error as SharedError;
use shared_http::api::responses::{CreateCacheResponse, DropCacheResponse, ValidationErrorResponse};
use carbon::ports::StorageFactory;
use storage_engine::UnifiedStorageFactory;
use tracing::info;

/// POST /admin/caches
pub async fn create_cache(
    State(state): State<AppState>,
    req: Request<Body>,
) -> Result<Json<CreateCacheResponse>, (StatusCode, Json<ValidationErrorResponse>)> {
    if let Some(raft) = &state.raft_node {
        if !raft.is_leader() {
            let resp = forward_to_leader(raft, req).await;
            let status =
                StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
            let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
                .await
                .unwrap_or_default();
            let json: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
            let created = json.get("created").and_then(|v| v.as_bool()).unwrap_or(false);
            let message =
                json.get("message").and_then(|v| v.as_str()).unwrap_or("").to_string();
            return if status.is_success() {
                Ok(Json(CreateCacheResponse { created, message }))
            } else {
                Err((status, Json(ValidationErrorResponse { error: message, field: None, details: None })))
            };
        }
    }

    let body_bytes = axum::body::to_bytes(req.into_body(), usize::MAX)
        .await
        .map_err(|_| {
            (StatusCode::BAD_REQUEST, Json(ValidationErrorResponse {
                error: "Failed to read request body".to_string(),
                field: None,
                details: None,
            }))
        })?;
    let req_body: CreateCacheRequest = serde_json::from_slice(&body_bytes).map_err(|e| {
        (StatusCode::BAD_REQUEST, Json(ValidationErrorResponse {
            error: e.to_string(),
            field: None,
            details: None,
        }))
    })?;

    info!("CREATE_CACHE: name={}, backend={}", req_body.name, req_body.eviction);

    let config = CacheConfigFactory::from_request(req_body).map_err(|err| {
        (StatusCode::BAD_REQUEST, Json(ValidationErrorResponse {
            error: err.to_string(),
            field: None,
            details: Some(format!("{:?}", err)),
        }))
    })?;

    let store = UnifiedStorageFactory.create_from_config(&config);

    match state.admin_ops.create_cache(config, store).await {
        Ok(resp) => Ok(Json(CreateCacheResponse { created: resp.created, message: resp.message })),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ValidationErrorResponse {
                error: format!("create_cache failed: {e}"),
                field: None,
                details: None,
            }),
        )),
    }
}

/// DELETE /admin/caches/:name
pub async fn drop_cache(
    State(state): State<AppState>,
    Path(name): Path<String>,
    req: Request<Body>,
) -> Result<Json<DropCacheResponse>, StatusCode> {
    info!("DROP_CACHE: name={}", name);

    if let Some(raft) = &state.raft_node {
        if !raft.is_leader() {
            let resp = forward_to_leader(raft, req).await;
            let status =
                StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
            let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
                .await
                .unwrap_or_default();
            let json: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
            let dropped = json.get("dropped").and_then(|v| v.as_bool()).unwrap_or(false);
            return if status.is_success() {
                Ok(Json(DropCacheResponse { dropped }))
            } else {
                Err(status)
            };
        }
    }

    match state.admin_ops.drop_cache(&name).await {
        Ok(resp) => Ok(Json(DropCacheResponse { dropped: resp.dropped })),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// GET /admin/caches
pub async fn list_caches(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    info!("LIST_CACHES");

    match state.admin_ops.list_caches().await {
        Ok(resp) => serde_json::to_value(resp).map(Json).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// GET /admin/caches/:name
pub async fn describe_cache(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    info!("DESCRIBE_CACHE: name={}", name);

    match state.admin_ops.describe_cache(&name).await {
        Ok(resp) => serde_json::to_value(resp).map(Json).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR),
        Err(SharedError::CacheNotFound(_)) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}
