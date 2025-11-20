use axum::{
    extract::{Request, State},
    http::{header, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use base64::{engine::general_purpose::STANDARD, Engine};
use carbon::auth::{AuthService, User};
use std::sync::Arc;

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

/// Authentication middleware
pub async fn auth_middleware(
    State(auth_service): State<Arc<AuthService>>,
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

    // Extract credentials
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

    // Authenticate user
    let user = match auth_service.authenticate(&username, &password).await {
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

    // Attach user to request extensions
    request.extensions_mut().insert(user);

    Ok(next.run(request).await)
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
}
