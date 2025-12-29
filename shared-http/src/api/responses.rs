#[cfg(not(target_arch = "wasm32"))]
use carbon::auth::{Permission, Role, User};
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashSet;

// For WASM builds, define Permission locally
#[cfg(target_arch = "wasm32")]
pub type Permission = String;

/// Response body for successful login
#[derive(Debug, Serialize)]
pub struct LoginResponse {
    /// The session token to use for logout
    pub token: String,
    /// Session expiration time in seconds from now
    pub expires_in: u64,
    /// Username of the authenticated user
    pub username: String,
}

impl From<Value> for LoginResponse {
    fn from(user: Value) -> Self {
        Self {
            token: user
                .get("token")
                .unwrap_or_default()
                .as_str()
                .unwrap_or_default()
                .to_string(),
            expires_in: user
                .get("expires_in")
                .unwrap_or_default()
                .as_u64()
                .unwrap_or_default(),
            username: user
                .get("username")
                .unwrap_or_default()
                .as_str()
                .unwrap_or_default()
                .to_string(),
        }
    }
}

/// Response body for logout
#[derive(Debug, Serialize)]
pub struct LogoutResponse {
    /// Success message
    pub message: String,
}

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

#[cfg(not(target_arch = "wasm32"))]
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

#[cfg(not(target_arch = "wasm32"))]
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
