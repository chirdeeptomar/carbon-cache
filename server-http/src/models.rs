use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// === Cache Operation Models ===

#[derive(Deserialize)]
pub struct PutRequest {
    pub value: String,
    #[serde(default)]
    pub ttl_ms: u64,
}

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

// === Admin Operation Models ===

#[derive(Deserialize)]
pub struct CreateCacheRequest {
    pub name: String,
    #[serde(default = "default_eviction")]
    pub eviction: String,  // "moka", "bounded", or "hybrid"
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
