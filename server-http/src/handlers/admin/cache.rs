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
use carbon::domain::response::admin::{
    DescribeCacheResponse, DropCacheResponse, ListCachesResponse,
};
use carbon::domain::CacheInfo;
use carbon_raft::types::RaftLogEntry;
use shared_http::api::responses::{CreateCacheResponse, ValidationErrorResponse};
use tracing::info;

/// POST /admin/caches
pub async fn create_cache(
    State(state): State<AppState>,
    req: Request<Body>,
) -> Result<Json<CreateCacheResponse>, (StatusCode, Json<ValidationErrorResponse>)> {
    let raft = &state.raft_node;

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

    match raft.write(RaftLogEntry::CreateCache(config)).await {
        Ok(carbon_raft::types::RaftResponse::CacheCreated { created }) => {
            Ok(Json(CreateCacheResponse {
                created,
                message: if created {
                    "Cache created".to_string()
                } else {
                    "Cache already exists".to_string()
                },
            }))
        }
        Ok(_) => Ok(Json(CreateCacheResponse { created: false, message: "ok".to_string() })),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ValidationErrorResponse {
                error: format!("Raft write failed: {e}"),
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

    let raft = &state.raft_node;

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

    match raft.write(RaftLogEntry::DropCache { name }).await {
        Ok(carbon_raft::types::RaftResponse::CacheDropped { dropped }) => {
            Ok(Json(DropCacheResponse { dropped }))
        }
        Ok(_) => Ok(Json(DropCacheResponse { dropped: false })),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// GET /admin/caches
pub async fn list_caches(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    info!("LIST_CACHES");

    let sm = state.raft_node.state.read().await;
    let caches: Vec<CacheInfo> =
        sm.list_configs().into_iter().map(|c| CacheInfo::from_config(&c)).collect();
    let resp = ListCachesResponse::new(caches);
    serde_json::to_value(resp).map(Json).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// GET /admin/caches/:name
pub async fn describe_cache(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    info!("DESCRIBE_CACHE: name={}", name);

    let sm = state.raft_node.state.read().await;
    match sm.describe_config(&name) {
        Some(config) => {
            let resp = DescribeCacheResponse::new(CacheInfo::from_config(&config));
            serde_json::to_value(resp)
                .map(Json)
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
        }
        None => Err(StatusCode::NOT_FOUND),
    }
}
