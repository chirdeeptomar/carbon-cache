use crate::api::{
    AssignRolesRequest, ChangePasswordRequest, CreateUserRequest, ErrorResponse, ListUsersResponse,
    ResetPasswordRequest, UserResponse,
};
use crate::middleware::check_permission;
use crate::state::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Extension, Json,
};
use carbon::auth::{Permission, User};
use tracing::{error, info};

/// POST /admin/users - Create a new user
pub async fn create_user(
    State(state): State<AppState>,
    Extension(current_user): Extension<User>,
    Json(req): Json<CreateUserRequest>,
) -> Result<(StatusCode, Json<UserResponse>), (StatusCode, Json<ErrorResponse>)> {
    // Check if current user has ManageUsers permission
    if let Err(e) =
        check_permission(&state.auth_service, &current_user, Permission::ManageUsers).await
    {
        return Err((e, Json(ErrorResponse::new("Insufficient permissions"))));
    }

    info!(
        "CREATE_USER: username={}, requested_by={}",
        req.username, current_user.username
    );

    match state
        .user_service
        .create_user(req.username, req.password, req.role_ids)
        .await
    {
        Ok(user) => Ok((StatusCode::CREATED, Json(user.into()))),
        Err(e) => {
            error!("Failed to create user: {}", e);
            Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new(e.to_string())),
            ))
        }
    }
}

/// GET /admin/users - List all users
pub async fn list_users(
    State(state): State<AppState>,
    Extension(current_user): Extension<User>,
) -> Result<Json<ListUsersResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Check if current user has ManageUsers permission
    if let Err(e) =
        check_permission(&state.auth_service, &current_user, Permission::ManageUsers).await
    {
        return Err((e, Json(ErrorResponse::new("Insufficient permissions"))));
    }

    match state.user_service.list_users().await {
        Ok(users) => {
            let user_responses = users.into_iter().map(|u| u.into()).collect();
            Ok(Json(ListUsersResponse {
                users: user_responses,
            }))
        }
        Err(e) => {
            error!("Failed to list users: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(e.to_string())),
            ))
        }
    }
}

/// GET /admin/users/{username} - Get user by username
pub async fn get_user(
    State(state): State<AppState>,
    Extension(current_user): Extension<User>,
    Path(username): Path<String>,
) -> Result<Json<UserResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Check if current user has ManageUsers permission
    if let Err(e) =
        check_permission(&state.auth_service, &current_user, Permission::ManageUsers).await
    {
        return Err((e, Json(ErrorResponse::new("Insufficient permissions"))));
    }

    match state.user_service.get_user(&username).await {
        Ok(user) => Ok(Json(user.into())),
        Err(e) => {
            error!("Failed to get user {}: {}", username, e);
            Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse::new(e.to_string())),
            ))
        }
    }
}

/// PUT /admin/users/{username}/roles - Assign roles to user
pub async fn assign_roles(
    State(state): State<AppState>,
    Extension(current_user): Extension<User>,
    Path(username): Path<String>,
    Json(req): Json<AssignRolesRequest>,
) -> Result<Json<UserResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Check if current user has ManageUsers permission
    if let Err(e) =
        check_permission(&state.auth_service, &current_user, Permission::ManageUsers).await
    {
        return Err((e, Json(ErrorResponse::new("Insufficient permissions"))));
    }

    info!(
        "ASSIGN_ROLES: username={}, requested_by={}",
        username, current_user.username
    );

    // Get user by username first
    let user = match state.user_service.get_user(&username).await {
        Ok(user) => user,
        Err(e) => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse::new(e.to_string())),
            ))
        }
    };

    match state
        .user_service
        .assign_roles(&user.id, req.role_ids)
        .await
    {
        Ok(user) => Ok(Json(user.into())),
        Err(e) => {
            error!("Failed to assign roles to {}: {}", username, e);
            Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new(e.to_string())),
            ))
        }
    }
}

/// PUT /admin/users/{username}/password - Change password
pub async fn change_password(
    State(state): State<AppState>,
    Extension(current_user): Extension<User>,
    Path(username): Path<String>,
    Json(req): Json<ChangePasswordRequest>,
) -> Result<Json<UserResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Users can change their own password, or admins can change any password
    let is_self = current_user.username == username;
    let has_manage_users =
        check_permission(&state.auth_service, &current_user, Permission::ManageUsers)
            .await
            .is_ok();

    if !is_self && !has_manage_users {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse::new("Insufficient permissions")),
        ));
    }

    info!(
        "CHANGE_PASSWORD: username={}, requested_by={}",
        username, current_user.username
    );

    // Get user by username first
    let user = match state.user_service.get_user(&username).await {
        Ok(user) => user,
        Err(e) => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse::new(e.to_string())),
            ))
        }
    };

    match state
        .user_service
        .change_password(&user.id, &req.old_password, &req.new_password)
        .await
    {
        Ok(user) => Ok(Json(user.into())),
        Err(e) => {
            error!("Failed to change password for {}: {}", username, e);
            Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new(e.to_string())),
            ))
        }
    }
}

/// PUT /admin/users/{username}/reset-password - Reset password (admin only)
pub async fn reset_password(
    State(state): State<AppState>,
    Extension(current_user): Extension<User>,
    Path(username): Path<String>,
    Json(req): Json<ResetPasswordRequest>,
) -> Result<Json<UserResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Check if current user has ManageUsers permission
    if let Err(e) =
        check_permission(&state.auth_service, &current_user, Permission::ManageUsers).await
    {
        return Err((e, Json(ErrorResponse::new("Insufficient permissions"))));
    }

    info!(
        "RESET_PASSWORD: username={}, requested_by={}",
        username, current_user.username
    );

    // Get user by username first
    let user = match state.user_service.get_user(&username).await {
        Ok(user) => user,
        Err(e) => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse::new(e.to_string())),
            ))
        }
    };

    match state
        .user_service
        .reset_password(&user.id, req.new_password)
        .await
    {
        Ok(user) => Ok(Json(user.into())),
        Err(e) => {
            error!("Failed to reset password for {}: {}", username, e);
            Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new(e.to_string())),
            ))
        }
    }
}

/// DELETE /admin/users/{username} - Delete user
pub async fn delete_user(
    State(state): State<AppState>,
    Extension(current_user): Extension<User>,
    Path(username): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    // Check if current user has ManageUsers permission
    if let Err(e) =
        check_permission(&state.auth_service, &current_user, Permission::ManageUsers).await
    {
        return Err((e, Json(ErrorResponse::new("Insufficient permissions"))));
    }

    info!(
        "DELETE_USER: username={}, requested_by={}",
        username, current_user.username
    );

    // Get user by username first
    let user = match state.user_service.get_user(&username).await {
        Ok(user) => user,
        Err(e) => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse::new(e.to_string())),
            ))
        }
    };

    match state
        .user_service
        .delete_user(&user.id, &current_user.id)
        .await
    {
        Ok(_) => Ok(StatusCode::NO_CONTENT),
        Err(e) => {
            error!("Failed to delete user {}: {}", username, e);
            Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new(e.to_string())),
            ))
        }
    }
}
