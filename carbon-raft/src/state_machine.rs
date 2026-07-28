use std::collections::BTreeMap;

use openraft::{LogId, StoredMembership};
use serde::{Deserialize, Serialize};

use carbon::auth::{Role, User};
use carbon::domain::CacheConfig;

use crate::types::{CarbonNode, NodeId};

/// The replicated state machine — the single source of truth for all cluster data.
///
/// Every node maintains an identical in-memory copy, kept in sync by applying
/// committed Raft log entries in order. Reads are served directly from this
/// struct; writes must go through Raft consensus first.
///
/// `BTreeMap` is used throughout so that snapshot serialization order is
/// deterministic, which keeps snapshot bytes identical across nodes.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct CacheStateMachine {
    /// The log ID of the last entry applied to this state machine.
    pub last_applied_log: Option<LogId<NodeId>>,
    /// The cluster membership config as of the last applied membership entry.
    pub last_membership: StoredMembership<NodeId, CarbonNode>,
    /// Cache data: `cache_name → (key → value)`.
    pub data: BTreeMap<String, BTreeMap<Vec<u8>, Vec<u8>>>,
    /// Cache configuration metadata, keyed by cache name.
    pub configs: BTreeMap<String, CacheConfig>,
    /// Auth: users keyed by user ID.
    pub users: BTreeMap<String, User>,
    /// Auth: user ID keyed by username (secondary index for fast username lookup).
    pub users_by_username: BTreeMap<String, String>,
    /// Auth: roles keyed by role ID.
    pub roles: BTreeMap<String, Role>,
    /// Auth: role ID keyed by role name (secondary index).
    pub roles_by_name: BTreeMap<String, String>,
}

impl CacheStateMachine {
    pub fn get(&self, cache_name: &str, key: &[u8]) -> Option<Vec<u8>> {
        self.data.get(cache_name)?.get(key).cloned()
    }

    pub fn list_configs(&self) -> Vec<CacheConfig> {
        self.configs.values().cloned().collect()
    }

    pub fn describe_config(&self, name: &str) -> Option<CacheConfig> {
        self.configs.get(name).cloned()
    }

    // --- User reads ---

    pub fn get_user_by_id(&self, id: &str) -> Option<&User> {
        self.users.get(id)
    }

    pub fn get_user_by_username(&self, username: &str) -> Option<&User> {
        let id = self.users_by_username.get(username)?;
        self.users.get(id)
    }

    pub fn username_exists(&self, username: &str) -> bool {
        self.users_by_username.contains_key(username)
    }

    pub fn list_users(&self) -> Vec<User> {
        self.users.values().cloned().collect()
    }

    // --- Role reads ---

    pub fn get_role_by_id(&self, id: &str) -> Option<&Role> {
        self.roles.get(id)
    }

    pub fn get_role_by_name(&self, name: &str) -> Option<&Role> {
        let id = self.roles_by_name.get(name)?;
        self.roles.get(id)
    }

    pub fn role_name_exists(&self, name: &str) -> bool {
        self.roles_by_name.contains_key(name)
    }

    pub fn list_roles(&self) -> Vec<Role> {
        self.roles.values().cloned().collect()
    }

    pub fn get_roles_by_ids(&self, ids: &[String]) -> Vec<Role> {
        ids.iter()
            .filter_map(|id| self.roles.get(id))
            .cloned()
            .collect()
    }
}
