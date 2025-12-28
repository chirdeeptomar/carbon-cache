use axum::{http::StatusCode, Json};

use crate::api::HealthResponse;

/// GET /health
pub async fn health_check() -> Result<Json<HealthResponse>, StatusCode> {
    Ok(Json(HealthResponse {
        message: "OK".into(),
    }))
}
