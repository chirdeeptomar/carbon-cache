use crate::middleware::check_permission;
use crate::state::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Extension, Json,
};
use carbon::auth::{
    error::AuthError,
    password::{hash_password, verify_password},
    Permission, User,
};
use carbon_raft::types::RaftLogEntry;
use chrono::Utc;
use shared_http::api::{
    AssignRolesRequest, ChangePasswordRequest, CreateUserRequest, ErrorResponse, ListUsersResponse,
    ResetPasswordRequest, UserResponse,
};
use tracing::{error, info};

fn auth_err(e: AuthError) -> (StatusCode, Json<ErrorResponse>) {
    (StatusCode::BAD_REQUEST, Json(ErrorResponse::new(e.to_string())))
}

fn not_found(msg: &str) -> (StatusCode, Json<ErrorResponse>) {
    (StatusCode::NOT_FOUND, Json(ErrorResponse::new(msg)))
}

fn internal(msg: impl ToString) -> (StatusCode, Json<ErrorResponse>) {
    (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse::new(msg.to_string())))
}

/// POST /admin/users
pub async fn create_user(
    State(state): State<AppState>,
    Extension(current_user): Extension<User>,
    Json(req): Json<CreateUserRequest>,
) -> Result<(StatusCode, Json<UserResponse>), (StatusCode, Json<ErrorResponse>)> {
    if let Err(e) =
        check_permission(&state.auth_service, &current_user, Permission::ManageUsers).await
    {
        return Err((e, Json(ErrorResponse::new("Insufficient permissions"))));
    }

    info!("CREATE_USER: username={}, requested_by={}", req.username, current_user.username);

    let raft = &state.raft_node;
    if raft.username_exists(&req.username).await {
        return Err(auth_err(AuthError::UserAlreadyExists));
    }
    let found = raft.get_roles_by_ids(&req.role_ids).await;
    if found.len() != req.role_ids.len() {
        return Err(auth_err(AuthError::RoleNotFound));
    }
    let password_hash = hash_password(&req.password).map_err(auth_err)?;
    let user = User::new(req.username, password_hash, req.role_ids);
    match raft.write(RaftLogEntry::CreateUser(user)).await {
        Ok(carbon_raft::types::RaftResponse::UserCreated { user }) => {
            Ok((StatusCode::CREATED, Json(user.into())))
        }
        Ok(_) => Err(internal("unexpected raft response")),
        Err(e) => {
            error!("Raft write CreateUser failed: {e}");
            Err(internal(e))
        }
    }
}

/// GET /admin/users
pub async fn list_users(
    State(state): State<AppState>,
    Extension(current_user): Extension<User>,
) -> Result<Json<ListUsersResponse>, (StatusCode, Json<ErrorResponse>)> {
    if let Err(e) =
        check_permission(&state.auth_service, &current_user, Permission::ManageUsers).await
    {
        return Err((e, Json(ErrorResponse::new("Insufficient permissions"))));
    }

    let users: Vec<UserResponse> =
        state.raft_node.list_users().await.into_iter().map(|u| u.into()).collect();
    Ok(Json(ListUsersResponse { users }))
}

/// GET /admin/users/{username}
pub async fn get_user(
    State(state): State<AppState>,
    Extension(current_user): Extension<User>,
    Path(username): Path<String>,
) -> Result<Json<UserResponse>, (StatusCode, Json<ErrorResponse>)> {
    if let Err(e) =
        check_permission(&state.auth_service, &current_user, Permission::ManageUsers).await
    {
        return Err((e, Json(ErrorResponse::new("Insufficient permissions"))));
    }

    match state.raft_node.get_user_by_username(&username).await {
        Some(user) => Ok(Json(user.into())),
        None => Err(not_found("User not found")),
    }
}

/// PUT /admin/users/{username}/roles
pub async fn assign_roles(
    State(state): State<AppState>,
    Extension(current_user): Extension<User>,
    Path(username): Path<String>,
    Json(req): Json<AssignRolesRequest>,
) -> Result<Json<UserResponse>, (StatusCode, Json<ErrorResponse>)> {
    if let Err(e) =
        check_permission(&state.auth_service, &current_user, Permission::ManageUsers).await
    {
        return Err((e, Json(ErrorResponse::new("Insufficient permissions"))));
    }

    info!("ASSIGN_ROLES: username={}, requested_by={}", username, current_user.username);

    let raft = &state.raft_node;
    let mut user = raft
        .get_user_by_username(&username)
        .await
        .ok_or_else(|| not_found("User not found"))?;

    let found = raft.get_roles_by_ids(&req.role_ids).await;
    if found.len() != req.role_ids.len() {
        return Err(auth_err(AuthError::RoleNotFound));
    }
    user.role_ids = req.role_ids;
    user.updated_at = Utc::now();

    match raft.write(RaftLogEntry::UpdateUser(user)).await {
        Ok(carbon_raft::types::RaftResponse::UserUpdated { user }) => Ok(Json(user.into())),
        Ok(_) => Err(internal("unexpected raft response")),
        Err(e) => {
            error!("Raft write UpdateUser failed: {e}");
            Err(internal(e))
        }
    }
}

/// PUT /admin/users/{username}/password
pub async fn change_password(
    State(state): State<AppState>,
    Extension(current_user): Extension<User>,
    Path(username): Path<String>,
    Json(req): Json<ChangePasswordRequest>,
) -> Result<Json<UserResponse>, (StatusCode, Json<ErrorResponse>)> {
    let is_self = current_user.username == username;
    let has_manage_users =
        check_permission(&state.auth_service, &current_user, Permission::ManageUsers)
            .await
            .is_ok();

    if !is_self && !has_manage_users {
        return Err((StatusCode::FORBIDDEN, Json(ErrorResponse::new("Insufficient permissions"))));
    }

    info!("CHANGE_PASSWORD: username={}, requested_by={}", username, current_user.username);

    let raft = &state.raft_node;
    let mut user = raft
        .get_user_by_username(&username)
        .await
        .ok_or_else(|| not_found("User not found"))?;

    let valid = verify_password(&req.old_password, &user.password_hash).map_err(auth_err)?;
    if !valid {
        return Err(auth_err(AuthError::InvalidCredentials));
    }
    user.password_hash = hash_password(&req.new_password).map_err(auth_err)?;
    user.updated_at = Utc::now();

    match raft.write(RaftLogEntry::UpdateUser(user)).await {
        Ok(carbon_raft::types::RaftResponse::UserUpdated { user }) => Ok(Json(user.into())),
        Ok(_) => Err(internal("unexpected raft response")),
        Err(e) => {
            error!("Raft write UpdateUser (change_password) failed: {e}");
            Err(internal(e))
        }
    }
}

/// PUT /admin/users/{username}/reset-password
pub async fn reset_password(
    State(state): State<AppState>,
    Extension(current_user): Extension<User>,
    Path(username): Path<String>,
    Json(req): Json<ResetPasswordRequest>,
) -> Result<Json<UserResponse>, (StatusCode, Json<ErrorResponse>)> {
    if let Err(e) =
        check_permission(&state.auth_service, &current_user, Permission::ManageUsers).await
    {
        return Err((e, Json(ErrorResponse::new("Insufficient permissions"))));
    }

    info!("RESET_PASSWORD: username={}, requested_by={}", username, current_user.username);

    let raft = &state.raft_node;
    let mut user = raft
        .get_user_by_username(&username)
        .await
        .ok_or_else(|| not_found("User not found"))?;

    user.password_hash = hash_password(&req.new_password).map_err(auth_err)?;
    user.updated_at = Utc::now();

    match raft.write(RaftLogEntry::UpdateUser(user)).await {
        Ok(carbon_raft::types::RaftResponse::UserUpdated { user }) => Ok(Json(user.into())),
        Ok(_) => Err(internal("unexpected raft response")),
        Err(e) => {
            error!("Raft write UpdateUser (reset_password) failed: {e}");
            Err(internal(e))
        }
    }
}

/// DELETE /admin/users/{username}
pub async fn delete_user(
    State(state): State<AppState>,
    Extension(current_user): Extension<User>,
    Path(username): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    if let Err(e) =
        check_permission(&state.auth_service, &current_user, Permission::ManageUsers).await
    {
        return Err((e, Json(ErrorResponse::new("Insufficient permissions"))));
    }

    info!("DELETE_USER: username={}, requested_by={}", username, current_user.username);

    let raft = &state.raft_node;
    let user = raft
        .get_user_by_username(&username)
        .await
        .ok_or_else(|| not_found("User not found"))?;

    if user.id == current_user.id {
        return Err(auth_err(AuthError::CannotDeleteSelf));
    }

    match raft.write(RaftLogEntry::DeleteUser { id: user.id }).await {
        Ok(_) => Ok(StatusCode::NO_CONTENT),
        Err(e) => {
            error!("Raft write DeleteUser failed: {e}");
            Err(internal(e))
        }
    }
}
