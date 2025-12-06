use axum::{
    extract::{Request, State, ConnectInfo},
    http::{header, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use base64::{engine::general_purpose::STANDARD, Engine};
use carbon::auth::{current_timestamp_ms, AuthService, MokaSessionRepository, SessionStore, User};
use std::net::SocketAddr;
use std::sync::Arc;

/// Shared state for authentication middleware
#[derive(Clone)]
pub struct AuthMiddlewareState {
    pub auth_service: Arc<AuthService>,
    pub session_store: Arc<SessionStore<MokaSessionRepository>>,
}

/// Authentication middleware with session support
pub async fn auth_middleware(
    State(state): State<AuthMiddlewareState>,
    mut request: Request,
    next: Next,
) -> Result<Response, Response> {
    // Get Authorization header
    let auth_header = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok());

    let auth_header = match auth_header {
        Some(h) => h,
        None => {
            return Err((
                StatusCode::UNAUTHORIZED,
                [(header::WWW_AUTHENTICATE, "Basic realm=\"Carbon Cache\"")],
                "Missing Authorization header",
            )
                .into_response())
        }
    };

    // Try Bearer token first (fast path)
    if let Some(token) = extract_bearer_token(auth_header) {
        match state.session_store.validate_session(&token).await {
            Ok(user) => {
                // Session valid - attach user and continue
                request.extensions_mut().insert(user);
                return Ok(next.run(request).await);
            }
            Err(_) => {
                // Invalid or expired session
                return Err((
                    StatusCode::UNAUTHORIZED,
                    [(header::WWW_AUTHENTICATE, "Bearer realm=\"Carbon Cache\"")],
                    "Invalid or expired session token",
                )
                    .into_response());
            }
        }
    }

    // Fallback to Basic Auth (slow path)
    let (username, password) = match extract_basic_auth(auth_header) {
        Some(creds) => creds,
        None => {
            return Err((
                StatusCode::UNAUTHORIZED,
                [(header::WWW_AUTHENTICATE, "Basic realm=\"Carbon Cache\"")],
                "Invalid Authorization header format",
            )
                .into_response())
        }
    };

    // Extract client IP address from headers or connection info
    let client_ip = extract_client_ip(&request);

    // Authenticate user with full Argon2 verification
    let user = match state.auth_service.authenticate(&username, &password).await {
        Ok(user) => user,
        Err(_) => {
            return Err((
                StatusCode::UNAUTHORIZED,
                [(header::WWW_AUTHENTICATE, "Basic realm=\"Carbon Cache\"")],
                "Invalid credentials",
            )
                .into_response())
        }
    };

    // Get or create session for this user (transparent session management)
    // If user has existing sessions, returns most recently accessed one
    // Otherwise creates new session (1 hour TTL)
    let (session, session_reused) = match state
        .session_store
        .get_or_create_user_session(user.clone(), 3600000, client_ip)
        .await
    {
        Ok(session) => {
            // Check if session was created just now or reused
            let reused = (current_timestamp_ms() - session.created_at) > 1000; // >1s old = reused
            (session, reused)
        }
        Err(_) => {
            // Failed to create/get session, but auth succeeded - continue without session
            request.extensions_mut().insert(user);
            return Ok(next.run(request).await);
        }
    };

    // Attach user to request extensions
    request.extensions_mut().insert(user);

    // Return response with session token and reuse indicator
    let mut response = next.run(request).await;
    response
        .headers_mut()
        .insert("X-Session-Token", session.token.parse().unwrap());

    // Transparency header - indicates if session was reused or created
    response
        .headers_mut()
        .insert("X-Session-Reused", if session_reused { "true" } else { "false" }.parse().unwrap());

    Ok(response)
}

/// Extract client IP address from request
/// Checks X-Forwarded-For header first, then X-Real-IP, then connection info
fn extract_client_ip(request: &Request) -> Option<String> {
    // Try X-Forwarded-For header (proxy/load balancer)
    if let Some(forwarded_for) = request.headers().get("X-Forwarded-For") {
        if let Ok(value) = forwarded_for.to_str() {
            // X-Forwarded-For can contain multiple IPs, take the first one
            if let Some(ip) = value.split(',').next() {
                return Some(ip.trim().to_string());
            }
        }
    }

    // Try X-Real-IP header (some proxies)
    if let Some(real_ip) = request.headers().get("X-Real-IP") {
        if let Ok(value) = real_ip.to_str() {
            return Some(value.to_string());
        }
    }

    // Try to get from connection info (direct connection)
    if let Some(connect_info) = request.extensions().get::<ConnectInfo<SocketAddr>>() {
        return Some(connect_info.0.ip().to_string());
    }

    None
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

/// Extract authenticated user from request extensions
pub fn get_authenticated_user(request: &Request) -> Option<&User> {
    request.extensions().get::<User>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_basic_auth() {
        // Test valid Basic Auth
        let header = format!("Basic {}", STANDARD.encode("admin:password123"));
        let result = extract_basic_auth(&header);
        assert!(result.is_some());
        let (username, password) = result.unwrap();
        assert_eq!(username, "admin");
        assert_eq!(password, "password123");

        // Test invalid format
        assert!(extract_basic_auth("Bearer token123").is_none());
        assert!(extract_basic_auth("Basic").is_none());
        assert!(extract_basic_auth("invalid").is_none());
    }

    #[test]
    fn test_extract_basic_auth_with_colon_in_password() {
        // Password can contain colons
        let header = format!("Basic {}", STANDARD.encode("admin:pass:word:123"));
        let result = extract_basic_auth(&header);
        assert!(result.is_some());
        let (username, password) = result.unwrap();
        assert_eq!(username, "admin");
        assert_eq!(password, "pass:word:123");
    }

    #[test]
    fn test_extract_bearer_token() {
        // Test valid Bearer token
        let header = "Bearer abc123def456";
        let result = extract_bearer_token(header);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "abc123def456");

        // Test invalid format
        assert!(extract_bearer_token("Basic abc123").is_none());
        assert!(extract_bearer_token("Bearer").is_none());
        assert!(extract_bearer_token("invalid").is_none());
    }
}
