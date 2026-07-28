use axum::{Json, http::StatusCode};

use shared_http::api::HealthResponse;

/// GET /health
pub async fn health_check() -> Result<Json<HealthResponse>, StatusCode> {
    Ok(Json(HealthResponse {
        message: "OK".into(),
    }))
}
