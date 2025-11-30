use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use carbon::auth::{AuthService, Permission, User};
use std::sync::Arc;

/// Check if authenticated user has required permission
pub async fn require_permission(
    permission: Permission,
    auth_service: Arc<AuthService>,
    request: Request,
    next: Next,
) -> Result<Response, Response> {
    // Get user from request extensions (set by auth middleware)
    let user = match extract_user_from_request(&request) {
        Ok(value) => value,
        Err(value) => return value,
    };

    // Check if user has permission
    match auth_service.authorize(user, permission).await {
        Ok(_) => Ok(next.run(request).await),
        Err(_) => Err((StatusCode::FORBIDDEN, "Insufficient permissions").into_response()),
    }
}

/// Check if authenticated user has any of the required permissions
pub async fn require_any_permission(
    permissions: Vec<Permission>,
    auth_service: Arc<AuthService>,
    request: Request,
    next: Next,
) -> Result<Response, Response> {
    // Get user from request extensions (set by auth middleware)
    let user = match extract_user_from_request(&request) {
        Ok(value) => value,
        Err(value) => return value,
    };

    // Check if user has any of the permissions
    match auth_service.has_any_permission(user, &permissions).await {
        Ok(_) => Ok(next.run(request).await),
        Err(_) => Err((StatusCode::FORBIDDEN, "Insufficient permissions").into_response()),
    }
}

/// Extract the User from the Request object
fn extract_user_from_request(request: &Request) -> Result<&User, Result<Response, Response>> {
    Ok(match request.extensions().get::<User>() {
        Some(user) => user,
        None => {
            return Err(Err(
                (StatusCode::UNAUTHORIZED, "Authentication required").into_response()
            ))
        }
    })
}

/// Middleware factory for requiring a specific permission
pub fn permission_layer(
    permission: Permission,
) -> impl Fn(
    State<Arc<AuthService>>,
    Request,
    Next,
)
    -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Response, Response>> + Send>>
       + Clone {
    move |State(auth_service): State<Arc<AuthService>>, request: Request, next: Next| {
        let perm = permission.clone();
        Box::pin(async move { require_permission(perm, auth_service, request, next).await })
    }
}

/// Helper function to check permissions in route handlers
pub async fn check_permission(
    auth_service: &AuthService,
    user: &User,
    permission: Permission,
) -> Result<(), StatusCode> {
    auth_service
        .authorize(user, permission)
        .await
        .map_err(|_| StatusCode::FORBIDDEN)
}

/// Helper function to check if user has any of the permissions in route handlers
pub async fn check_any_permission(
    auth_service: &AuthService,
    user: &User,
    permissions: &[Permission],
) -> Result<(), StatusCode> {
    auth_service
        .has_any_permission(user, permissions)
        .await
        .map_err(|_| StatusCode::FORBIDDEN)
}
