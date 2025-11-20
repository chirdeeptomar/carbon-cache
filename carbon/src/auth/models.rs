use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Permission {
    // Cache operations
    ReadCache,
    WriteCache,
    DeleteCache,

    // Admin cache operations
    AdminRead,
    AdminWrite,
    AdminDelete,

    // User management
    ManageUsers,

    // Role management
    ManageRoles,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Role {
    pub id: String,
    pub name: String,
    pub permissions: HashSet<Permission>,
    pub is_system_role: bool,
    pub created_at: DateTime<Utc>,
}

impl Role {
    pub fn new(name: String, permissions: HashSet<Permission>, is_system_role: bool) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name,
            permissions,
            is_system_role,
            created_at: Utc::now(),
        }
    }

    pub fn has_permission(&self, permission: &Permission) -> bool {
        self.permissions.contains(permission)
    }

    pub fn has_any_permission(&self, permissions: &[Permission]) -> bool {
        permissions.iter().any(|p| self.permissions.contains(p))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub username: String,
    pub password_hash: String,
    pub role_ids: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl User {
    pub fn new(username: String, password_hash: String, role_ids: Vec<String>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            username,
            password_hash,
            role_ids,
            created_at: now,
            updated_at: now,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_role_has_permission() {
        let mut permissions = HashSet::new();
        permissions.insert(Permission::ReadCache);
        permissions.insert(Permission::WriteCache);

        let role = Role::new("test".to_string(), permissions, false);

        assert!(role.has_permission(&Permission::ReadCache));
        assert!(role.has_permission(&Permission::WriteCache));
        assert!(!role.has_permission(&Permission::DeleteCache));
    }

    #[test]
    fn test_role_has_any_permission() {
        let mut permissions = HashSet::new();
        permissions.insert(Permission::ReadCache);

        let role = Role::new("test".to_string(), permissions, false);

        assert!(role.has_any_permission(&[Permission::ReadCache, Permission::WriteCache]));
        assert!(!role.has_any_permission(&[Permission::WriteCache, Permission::DeleteCache]));
    }
}
