#[cfg(not(target_arch = "wasm32"))]
use carbon::auth::Permission;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};

// For WASM builds, define Permission locally
#[cfg(target_arch = "wasm32")]
pub type Permission = String;

/// Request body for login endpoint
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub password: String,
    pub role_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct AssignRolesRequest {
    pub role_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct ChangePasswordRequest {
    pub old_password: String,
    pub new_password: String,
}

#[derive(Debug, Deserialize)]
pub struct ResetPasswordRequest {
    pub new_password: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateRoleRequest {
    pub name: String,
    pub permissions: HashSet<Permission>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateRoleRequest {
    pub permissions: HashSet<Permission>,
}
// === Cache Operation Models ===

#[derive(Deserialize)]
pub struct PutRequest {
    pub value: String,
}

// === Admin Operation Models ===

#[derive(Deserialize)]
pub struct CreateCacheRequest {
    pub name: String,
    #[serde(default = "default_eviction")]
    pub eviction: String, // "moka", "bounded", or "hybrid"
    #[serde(default)]
    pub mem_bytes: Option<u64>,
    #[serde(default)]
    pub disk_path: Option<String>,
    #[serde(default)]
    pub shards: Option<u8>,
    #[serde(default)]
    pub policy: String,
    #[serde(default)]
    pub default_ttl_ms: Option<u64>,
    #[serde(default)]
    pub max_value_bytes: Option<u64>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub tags: Option<HashMap<String, String>>,
}

fn default_eviction() -> String {
    "timebound".to_string()
}
