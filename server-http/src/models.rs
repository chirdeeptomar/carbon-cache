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
    pub mem_bytes: u64,
    #[serde(default)]
    pub disk_path: Option<String>,
    #[serde(default)]
    pub shards: u32,
    #[serde(default)]
    pub policy: String,
    #[serde(default)]
    pub default_ttl_ms: u64,
    #[serde(default)]
    pub max_value_bytes: u64,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub tags: Option<HashMap<String, String>>,
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
