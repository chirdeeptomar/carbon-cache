use crate::middleware::check_permission;
use crate::state::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Extension, Json,
};
use carbon::auth::{error::AuthError, models::Role, Permission, User};
use carbon_raft::types::RaftLogEntry;
use shared_http::api::{
    CreateRoleRequest, ErrorResponse, ListRolesResponse, RoleResponse, UpdateRoleRequest,
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

/// POST /admin/roles
pub async fn create_role(
    State(state): State<AppState>,
    Extension(current_user): Extension<User>,
    Json(req): Json<CreateRoleRequest>,
) -> Result<(StatusCode, Json<RoleResponse>), (StatusCode, Json<ErrorResponse>)> {
    if let Err(e) =
        check_permission(&state.auth_service, &current_user, Permission::ManageRoles).await
    {
        return Err((e, Json(ErrorResponse::new("Insufficient permissions"))));
    }

    info!("CREATE_ROLE: name={}, requested_by={}", req.name, current_user.username);

    let raft = &state.raft_node;
    if raft.get_role_by_name(&req.name).await.is_some() {
        return Err(auth_err(AuthError::RoleAlreadyExists));
    }
    let role = Role::new(req.name, req.permissions.into_iter().collect(), false);
    match raft.write(RaftLogEntry::CreateRole(role)).await {
        Ok(carbon_raft::types::RaftResponse::RoleCreated { role }) => {
            Ok((StatusCode::CREATED, Json(role.into())))
        }
        Ok(_) => Err(internal("unexpected raft response")),
        Err(e) => {
            error!("Raft write CreateRole failed: {e}");
            Err(internal(e))
        }
    }
}

/// GET /admin/roles
pub async fn list_roles(
    State(state): State<AppState>,
    Extension(current_user): Extension<User>,
) -> Result<Json<ListRolesResponse>, (StatusCode, Json<ErrorResponse>)> {
    if let Err(e) =
        check_permission(&state.auth_service, &current_user, Permission::AdminRead).await
    {
        return Err((e, Json(ErrorResponse::new("Insufficient permissions"))));
    }

    let roles: Vec<RoleResponse> =
        state.raft_node.list_roles().await.into_iter().map(|r| r.into()).collect();
    Ok(Json(ListRolesResponse { roles }))
}

/// GET /admin/roles/{name}
pub async fn get_role(
    State(state): State<AppState>,
    Extension(current_user): Extension<User>,
    Path(name): Path<String>,
) -> Result<Json<RoleResponse>, (StatusCode, Json<ErrorResponse>)> {
    if let Err(e) =
        check_permission(&state.auth_service, &current_user, Permission::AdminRead).await
    {
        return Err((e, Json(ErrorResponse::new("Insufficient permissions"))));
    }

    match state.raft_node.get_role_by_name(&name).await {
        Some(role) => Ok(Json(role.into())),
        None => Err(not_found("Role not found")),
    }
}

/// PUT /admin/roles/{name}
pub async fn update_role(
    State(state): State<AppState>,
    Extension(current_user): Extension<User>,
    Path(name): Path<String>,
    Json(req): Json<UpdateRoleRequest>,
) -> Result<Json<RoleResponse>, (StatusCode, Json<ErrorResponse>)> {
    if let Err(e) =
        check_permission(&state.auth_service, &current_user, Permission::ManageRoles).await
    {
        return Err((e, Json(ErrorResponse::new("Insufficient permissions"))));
    }

    info!("UPDATE_ROLE: name={}, requested_by={}", name, current_user.username);

    let raft = &state.raft_node;
    let mut role = raft
        .get_role_by_name(&name)
        .await
        .ok_or_else(|| not_found("Role not found"))?;

    if role.is_system_role {
        return Err(auth_err(AuthError::CannotDeleteSystemRole));
    }
    role.permissions = req.permissions.into_iter().collect();

    match raft.write(RaftLogEntry::UpdateRole(role)).await {
        Ok(carbon_raft::types::RaftResponse::RoleUpdated { role }) => Ok(Json(role.into())),
        Ok(_) => Err(internal("unexpected raft response")),
        Err(e) => {
            error!("Raft write UpdateRole failed: {e}");
            Err(internal(e))
        }
    }
}

/// DELETE /admin/roles/{name}
pub async fn delete_role(
    State(state): State<AppState>,
    Extension(current_user): Extension<User>,
    Path(name): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    if let Err(e) =
        check_permission(&state.auth_service, &current_user, Permission::ManageRoles).await
    {
        return Err((e, Json(ErrorResponse::new("Insufficient permissions"))));
    }

    info!("DELETE_ROLE: name={}, requested_by={}", name, current_user.username);

    let raft = &state.raft_node;
    let role = raft
        .get_role_by_name(&name)
        .await
        .ok_or_else(|| not_found("Role not found"))?;

    if role.is_system_role {
        return Err(auth_err(AuthError::CannotDeleteSystemRole));
    }

    match raft.write(RaftLogEntry::DeleteRole { id: role.id }).await {
        Ok(_) => Ok(StatusCode::NO_CONTENT),
        Err(e) => {
            error!("Raft write DeleteRole failed: {e}");
            Err(internal(e))
        }
    }
}
