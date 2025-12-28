use carbon::auth::{Permission, Role, User};
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::collections::HashSet;

#[derive(Serialize)]
pub struct HealthResponse {
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct UserResponse {
    pub id: String,
    pub username: String,
    pub role_ids: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<User> for UserResponse {
    fn from(user: User) -> Self {
        Self {
            id: user.id,
            username: user.username,
            role_ids: user.role_ids,
            created_at: user.created_at,
            updated_at: user.updated_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ListUsersResponse {
    pub users: Vec<UserResponse>,
}

#[derive(Debug, Serialize)]
pub struct RoleResponse {
    pub id: String,
    pub name: String,
    pub permissions: HashSet<Permission>,
    pub is_system_role: bool,
    pub created_at: DateTime<Utc>,
}

impl From<Role> for RoleResponse {
    fn from(role: Role) -> Self {
        Self {
            id: role.id,
            name: role.name,
            permissions: role.permissions,
            is_system_role: role.is_system_role,
            created_at: role.created_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ListRolesResponse {
    pub roles: Vec<RoleResponse>,
}

// Error response
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

impl ErrorResponse {
    pub fn new(error: impl Into<String>) -> Self {
        Self {
            error: error.into(),
        }
    }
}

// === Cache Operation Models ===

#[derive(Serialize)]
pub struct PutResponse {
    pub ok: bool,
}

#[derive(Serialize)]
pub struct GetResponse {
    pub found: bool,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub value: String,
    pub ttl_ms_remaining: u64,
}

#[derive(Serialize)]
pub struct DeleteResponse {
    pub deleted: bool,
}

#[derive(Serialize)]
pub struct CreateCacheResponse {
    pub created: bool,
    pub message: String,
}

#[derive(Serialize)]
pub struct DropCacheResponse {
    pub dropped: bool,
}

#[derive(Serialize)]
pub struct ValidationErrorResponse {
    pub error: String,
    pub field: Option<String>,
    pub details: Option<String>,
}
