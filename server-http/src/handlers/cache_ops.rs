use crate::models::{DeleteResponse, GetResponse, PutRequest, PutResponse};
use crate::state::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use bytes::Bytes;
use carbon::planes::data::operation::CacheOperations;
use tracing::info;

/// PUT /cache/:cache_name/:key
pub async fn put_value(
    State(state): State<AppState>,
    Path((cache_name, key)): Path<(String, String)>,
    Json(req): Json<PutRequest>,
) -> Result<Json<PutResponse>, StatusCode> {
    info!("PUT: cache={}, key={}", cache_name, key);

    let ttl = if req.ttl_ms > 0 {
        Some(shared::TtlMs(req.ttl_ms))
    } else {
        None
    };

    match state
        .cache_operations
        .put(&cache_name, key.into_bytes(), Bytes::from(req.value), ttl)
        .await
    {
        Ok(_) => Ok(Json(PutResponse { ok: true })),
        Err(shared::Error::CacheNotFound(_)) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// GET /cache/:cache_name/:key
pub async fn get_value(
    State(state): State<AppState>,
    Path((cache_name, key)): Path<(String, String)>,
) -> Result<Json<GetResponse>, StatusCode> {
    info!("GET: cache={}, key={}", cache_name, key);

    let key_bytes = key.into_bytes();

    match state.cache_operations.get(&cache_name, &key_bytes).await {
        Ok(result) => {
            let value = String::from_utf8(result.message.to_vec())
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

            Ok(Json(GetResponse {
                found: result.found,
                value,
                ttl_ms_remaining: 0,
            }))
        }
        Err(shared::Error::NotFound) => Ok(Json(GetResponse {
            found: false,
            value: String::new(),
            ttl_ms_remaining: 0,
        })),
        Err(shared::Error::CacheNotFound(_)) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// DELETE /cache/:cache_name/:key
pub async fn delete_value(
    State(state): State<AppState>,
    Path((cache_name, key)): Path<(String, String)>,
) -> Result<Json<DeleteResponse>, StatusCode> {
    info!("DELETE: cache={}, key={}", cache_name, key);

    let key_bytes = key.into_bytes();

    match state.cache_operations.delete(&cache_name, &key_bytes).await {
        Ok(result) => Ok(Json(DeleteResponse {
            deleted: result.deleted,
        })),
        Err(shared::Error::CacheNotFound(_)) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}
