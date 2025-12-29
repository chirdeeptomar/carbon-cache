use axum::{
    extract::State,
    http::{header, StatusCode},
    response::{IntoResponse, Json},
};
use base64::{engine::general_purpose::STANDARD, Engine};
use carbon::auth::{AuthService, MokaSessionRepository, SessionStore};
use shared_http::api::{LoginRequest, LoginResponse, LogoutResponse};
use std::sync::Arc;
/// State needed for auth handlers
#[derive(Clone)]
pub struct AuthHandlerState {
    pub auth_service: Arc<AuthService>,
    pub session_store: Arc<SessionStore<MokaSessionRepository>>,
}

/// POST /auth/login
///
/// Authenticate and get a session token (useful for logout later).
///
/// This endpoint accepts either:
/// 1. JSON body: {"username": "admin", "password": "admin123"}
/// 2. Basic Auth header: Authorization: Basic base64(username:password)
///
/// Returns a session token that can be used for logout.
/// Note: Regular API calls don't need to use this - they can just use Basic Auth
/// and sessions will be managed automatically.
pub async fn login(
    State(state): State<AuthHandlerState>,
    headers: axum::http::HeaderMap,
    body: Result<Json<LoginRequest>, axum::extract::rejection::JsonRejection>,
) -> Result<Json<LoginResponse>, impl IntoResponse> {
    // Try to extract credentials from either JSON body or Basic Auth header
    let (username, password) = match body {
        // Use JSON body credentials if available
        Ok(Json(login_req)) => (login_req.username, login_req.password),
        // If no valid JSON body, try Basic Auth header
        Err(_) => {
            if let Some(auth_header) = headers.get(header::AUTHORIZATION) {
                if let Ok(auth_str) = auth_header.to_str() {
                    if let Some((user, pass)) = extract_basic_auth(auth_str) {
                        (user, pass)
                    } else {
                        return Err((
                            StatusCode::BAD_REQUEST,
                            Json(serde_json::json!({
                                "error": "Invalid Authorization header format"
                            })),
                        ));
                    }
                } else {
                    return Err((
                        StatusCode::BAD_REQUEST,
                        Json(serde_json::json!({
                            "error": "Invalid Authorization header"
                        })),
                    ));
                }
            } else {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({
                        "error": "Missing credentials. Provide either JSON body or Basic Auth header"
                    })),
                ));
            }
        }
    };

    // Extract client IP address from headers only
    let client_ip = headers
        .get("X-Forwarded-For")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(|s| s.trim().to_string())
        .or_else(|| {
            headers
                .get("X-Real-IP")
                .and_then(|h| h.to_str().ok())
                .map(|s| s.to_string())
        });

    // Authenticate user with Argon2 verification
    let user = match state.auth_service.authenticate(&username, &password).await {
        Ok(user) => user,
        Err(_) => {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({
                    "error": "Invalid username or password"
                })),
            ));
        }
    };

    // Create session (1 hour TTL = 3600000 ms)
    let session = match state
        .session_store
        .create_session(user.clone(), 3600000, client_ip)
        .await
    {
        Ok(session) => session,
        Err(_) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Failed to create session"
                })),
            ));
        }
    };

    // Return session token
    Ok(Json(LoginResponse {
        token: session.token,
        expires_in: 3600, // 1 hour in seconds
        username: user.username,
    }))
}

/// POST /auth/logout
///
/// Invalidate a session token.
///
/// Accepts the session token in the Authorization header as Bearer token:
/// Authorization: Bearer <token>
pub async fn logout(
    State(state): State<AuthHandlerState>,
    headers: axum::http::HeaderMap,
) -> Result<Json<LogoutResponse>, impl IntoResponse> {
    // Get Authorization header
    let auth_header = headers
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({
                    "error": "Missing Authorization header"
                })),
            )
        })?;

    // Extract Bearer token
    let token = extract_bearer_token(auth_header).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "Invalid Authorization header format. Expected: Bearer <token>"
            })),
        )
    })?;

    // Invalidate the session
    match state.session_store.invalidate_session(&token).await {
        Ok(true) => Ok(Json(LogoutResponse {
            message: "Session logged out successfully".to_string(),
        })),
        Ok(false) => Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": "Session not found or already expired"
            })),
        )),
        Err(_) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": "Failed to logout session"
            })),
        )),
    }
}

/// Extract Basic Auth credentials from Authorization header
fn extract_basic_auth(auth_header: &str) -> Option<(String, String)> {
    // Authorization: Basic <base64>
    let parts: Vec<&str> = auth_header.split_whitespace().collect();

    if parts.len() != 2 || parts[0] != "Basic" {
        return None;
    }

    // Decode base64
    let decoded = STANDARD.decode(parts[1]).ok()?;
    let decoded_str = String::from_utf8(decoded).ok()?;

    // Split username:password
    let mut parts = decoded_str.splitn(2, ':');
    let username = parts.next()?.to_string();
    let password = parts.next()?.to_string();

    Some((username, password))
}

/// Extract Bearer token from Authorization header
fn extract_bearer_token(auth_header: &str) -> Option<String> {
    // Authorization: Bearer <token>
    let parts: Vec<&str> = auth_header.split_whitespace().collect();

    if parts.len() != 2 || parts[0] != "Bearer" {
        return None;
    }

    Some(parts[1].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_basic_auth() {
        let header = format!("Basic {}", STANDARD.encode("admin:password123"));
        let result = extract_basic_auth(&header);
        assert!(result.is_some());
        let (username, password) = result.unwrap();
        assert_eq!(username, "admin");
        assert_eq!(password, "password123");
    }

    #[test]
    fn test_extract_bearer_token() {
        let header = "Bearer abc123def456";
        let result = extract_bearer_token(header);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "abc123def456");
    }
}
