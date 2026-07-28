use axum::http::StatusCode;
use carbon::auth::{AuthService, Permission, User};

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
