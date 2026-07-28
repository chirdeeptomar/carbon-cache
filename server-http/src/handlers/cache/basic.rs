use crate::leader_proxy::forward_to_leader;
use crate::state::AppState;
use axum::{
    Json,
    extract::{Path, Request, State},
    http::StatusCode,
};
use bytes::Bytes;
use shared_http::api::{DeleteResponse, GetResponse, PutRequest, PutResponse};
use tracing::info;

/// PUT /cache/:cache_name/:key
pub async fn put_value(
    State(state): State<AppState>,
    Path((cache_name, key)): Path<(String, String)>,
    req: Request,
) -> Result<Json<PutResponse>, StatusCode> {
    info!("PUT: cache={}, key={}", cache_name, key);

    if let Some(raft) = &state.raft_node
        && !raft.is_leader()
    {
        let resp = forward_to_leader(raft, req).await;
        let status = resp.status();
        if status.is_success() {
            let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
                .await
                .map_err(|_| StatusCode::BAD_GATEWAY)?;
            let put_resp: PutResponse =
                serde_json::from_slice(&body).map_err(|_| StatusCode::BAD_GATEWAY)?;
            return Ok(Json(put_resp));
        } else {
            return Err(StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY));
        }
    }

    let body = axum::body::to_bytes(req.into_body(), usize::MAX)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    let req_body: PutRequest =
        serde_json::from_slice(&body).map_err(|_| StatusCode::BAD_REQUEST)?;

    let key_bytes = key.into_bytes();
    let value = Bytes::from(req_body.value);

    match state.cache_ops.put(&cache_name, key_bytes, value).await {
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

    match state.cache_ops.get(&cache_name, &key_bytes).await {
        Ok(result) => {
            let value = String::from_utf8(result.message.to_vec())
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            Ok(Json(GetResponse {
                found: result.found,
                value,
                ttl_ms_remaining: 0,
            }))
        }
        Err(shared::Error::KeyNotFound) => Ok(Json(GetResponse {
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
    req: Request,
) -> Result<Json<DeleteResponse>, StatusCode> {
    info!("DELETE: cache={}, key={}", cache_name, key);

    if let Some(raft) = &state.raft_node
        && !raft.is_leader()
    {
        let resp = forward_to_leader(raft, req).await;
        let status = resp.status();
        if status.is_success() {
            let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
                .await
                .map_err(|_| StatusCode::BAD_GATEWAY)?;
            let del_resp: DeleteResponse =
                serde_json::from_slice(&body).map_err(|_| StatusCode::BAD_GATEWAY)?;
            return Ok(Json(del_resp));
        } else {
            return Err(StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY));
        }
    }

    let key_bytes = key.into_bytes();

    match state.cache_ops.delete(&cache_name, &key_bytes).await {
        Ok(result) => Ok(Json(DeleteResponse {
            deleted: result.deleted,
        })),
        Err(shared::Error::CacheNotFound(_)) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}
