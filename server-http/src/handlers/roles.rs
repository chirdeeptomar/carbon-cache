use crate::dto::{
    CreateRoleRequest, ErrorResponse, ListRolesResponse, RoleResponse, UpdateRoleRequest,
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

/// POST /admin/roles - Create a new custom role
pub async fn create_role(
    State(state): State<AppState>,
    Extension(current_user): Extension<User>,
    Json(req): Json<CreateRoleRequest>,
) -> Result<(StatusCode, Json<RoleResponse>), (StatusCode, Json<ErrorResponse>)> {
    // Check if current user has ManageRoles permission
    if let Err(e) = check_permission(&state.auth_service, &current_user, Permission::ManageRoles)
        .await
    {
        return Err((e, Json(ErrorResponse::new("Insufficient permissions"))));
    }

    info!(
        "CREATE_ROLE: name={}, requested_by={}",
        req.name, current_user.username
    );

    match state.role_service.create_role(req.name, req.permissions).await {
        Ok(role) => Ok((StatusCode::CREATED, Json(role.into()))),
        Err(e) => {
            error!("Failed to create role: {}", e);
            Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new(e.to_string())),
            ))
        }
    }
}

/// GET /admin/roles - List all roles
pub async fn list_roles(
    State(state): State<AppState>,
    Extension(current_user): Extension<User>,
) -> Result<Json<ListRolesResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Check if current user has AdminRead permission (any user/admin can list roles)
    if let Err(e) = check_permission(&state.auth_service, &current_user, Permission::AdminRead)
        .await
    {
        return Err((e, Json(ErrorResponse::new("Insufficient permissions"))));
    }

    match state.role_service.list_roles().await {
        Ok(roles) => {
            let role_responses = roles.into_iter().map(|r| r.into()).collect();
            Ok(Json(ListRolesResponse {
                roles: role_responses,
            }))
        }
        Err(e) => {
            error!("Failed to list roles: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(e.to_string())),
            ))
        }
    }
}

/// GET /admin/roles/{name} - Get role by name
pub async fn get_role(
    State(state): State<AppState>,
    Extension(current_user): Extension<User>,
    Path(name): Path<String>,
) -> Result<Json<RoleResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Check if current user has AdminRead permission
    if let Err(e) = check_permission(&state.auth_service, &current_user, Permission::AdminRead)
        .await
    {
        return Err((e, Json(ErrorResponse::new("Insufficient permissions"))));
    }

    match state.role_service.get_role(&name).await {
        Ok(role) => Ok(Json(role.into())),
        Err(e) => {
            error!("Failed to get role {}: {}", name, e);
            Err((StatusCode::NOT_FOUND, Json(ErrorResponse::new(e.to_string()))))
        }
    }
}

/// PUT /admin/roles/{name} - Update role permissions
pub async fn update_role(
    State(state): State<AppState>,
    Extension(current_user): Extension<User>,
    Path(name): Path<String>,
    Json(req): Json<UpdateRoleRequest>,
) -> Result<Json<RoleResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Check if current user has ManageRoles permission
    if let Err(e) = check_permission(&state.auth_service, &current_user, Permission::ManageRoles)
        .await
    {
        return Err((e, Json(ErrorResponse::new("Insufficient permissions"))));
    }

    info!(
        "UPDATE_ROLE: name={}, requested_by={}",
        name, current_user.username
    );

    // Get role by name first
    let role = match state.role_service.get_role(&name).await {
        Ok(role) => role,
        Err(e) => {
            return Err((StatusCode::NOT_FOUND, Json(ErrorResponse::new(e.to_string()))))
        }
    };

    match state
        .role_service
        .update_role(&role.id, req.permissions)
        .await
    {
        Ok(role) => Ok(Json(role.into())),
        Err(e) => {
            error!("Failed to update role {}: {}", name, e);
            Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new(e.to_string())),
            ))
        }
    }
}

/// DELETE /admin/roles/{name} - Delete a custom role
pub async fn delete_role(
    State(state): State<AppState>,
    Extension(current_user): Extension<User>,
    Path(name): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    // Check if current user has ManageRoles permission
    if let Err(e) = check_permission(&state.auth_service, &current_user, Permission::ManageRoles)
        .await
    {
        return Err((e, Json(ErrorResponse::new("Insufficient permissions"))));
    }

    info!(
        "DELETE_ROLE: name={}, requested_by={}",
        name, current_user.username
    );

    // Get role by name first
    let role = match state.role_service.get_role(&name).await {
        Ok(role) => role,
        Err(e) => {
            return Err((StatusCode::NOT_FOUND, Json(ErrorResponse::new(e.to_string()))))
        }
    };

    match state.role_service.delete_role(&role.id).await {
        Ok(_) => Ok(StatusCode::NO_CONTENT),
        Err(e) => {
            error!("Failed to delete role {}: {}", name, e);
            Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new(e.to_string())),
            ))
        }
    }
}
