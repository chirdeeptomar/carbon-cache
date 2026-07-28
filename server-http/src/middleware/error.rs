use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};

/// Typed authentication/authorization errors for the HTTP middleware.
///
/// Each variant maps to a fixed status code, response body, and (for
/// authentication failures) a `WWW-Authenticate` challenge, so no call
/// site builds ad-hoc `(StatusCode, message)` tuples.
#[derive(thiserror::Error, Debug)]
pub enum AuthError {
    #[error("Missing Authorization header")]
    MissingHeader,
    #[error("Invalid Authorization header format")]
    MalformedHeader,
    #[error("Invalid or expired session token")]
    InvalidSession,
    #[error("Invalid credentials")]
    LoginFailed,
    #[error("Authentication required")]
    AuthRequired,
    #[error("Insufficient permissions")]
    PermissionDenied,
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let status = match self {
            AuthError::PermissionDenied => StatusCode::FORBIDDEN,
            _ => StatusCode::UNAUTHORIZED,
        };
        let challenge = match self {
            AuthError::InvalidSession => Some("Bearer realm=\"Carbon Cache\""),
            AuthError::PermissionDenied => None,
            _ => Some("Basic realm=\"Carbon Cache\""),
        };
        let mut res = (status, self.to_string()).into_response();
        if let Some(c) = challenge {
            res.headers_mut().insert(
                header::WWW_AUTHENTICATE,
                header::HeaderValue::from_static(c),
            );
        }
        res
    }
}
