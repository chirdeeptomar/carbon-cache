use super::error::AuthError;
use super::models::{Permission, Role, User};
use super::password::hash_password;
use std::collections::HashSet;

/// Create the default system roles
pub fn create_default_roles() -> Vec<Role> {
    vec![create_admin_role(), create_user_role(), create_readonly_role()]
}

/// Create the admin role with all permissions
pub fn create_admin_role() -> Role {
    let mut permissions = HashSet::new();
    permissions.insert(Permission::ReadCache);
    permissions.insert(Permission::WriteCache);
    permissions.insert(Permission::DeleteCache);
    permissions.insert(Permission::AdminRead);
    permissions.insert(Permission::AdminWrite);
    permissions.insert(Permission::AdminDelete);
    permissions.insert(Permission::ManageUsers);
    permissions.insert(Permission::ManageRoles);

    Role::new("admin".to_string(), permissions, true)
}

/// Create the user role with cache operations and read-only admin access
pub fn create_user_role() -> Role {
    let mut permissions = HashSet::new();
    permissions.insert(Permission::ReadCache);
    permissions.insert(Permission::WriteCache);
    permissions.insert(Permission::DeleteCache);
    permissions.insert(Permission::AdminRead);

    Role::new("user".to_string(), permissions, true)
}

/// Create the read-only role with only read permissions
pub fn create_readonly_role() -> Role {
    let mut permissions = HashSet::new();
    permissions.insert(Permission::ReadCache);
    permissions.insert(Permission::AdminRead);

    Role::new("read-only".to_string(), permissions, true)
}

/// Create the default admin user
pub fn create_default_admin(
    username: String,
    password: String,
    admin_role_id: String,
) -> Result<User, AuthError> {
    let password_hash = hash_password(&password)?;
    Ok(User::new(username, password_hash, vec![admin_role_id]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_admin_role_has_all_permissions() {
        let admin = create_admin_role();
        assert_eq!(admin.name, "admin");
        assert!(admin.is_system_role);
        assert!(admin.has_permission(&Permission::ReadCache));
        assert!(admin.has_permission(&Permission::WriteCache));
        assert!(admin.has_permission(&Permission::DeleteCache));
        assert!(admin.has_permission(&Permission::AdminRead));
        assert!(admin.has_permission(&Permission::AdminWrite));
        assert!(admin.has_permission(&Permission::AdminDelete));
        assert!(admin.has_permission(&Permission::ManageUsers));
        assert!(admin.has_permission(&Permission::ManageRoles));
    }

    #[test]
    fn test_user_role_permissions() {
        let user = create_user_role();
        assert_eq!(user.name, "user");
        assert!(user.is_system_role);
        assert!(user.has_permission(&Permission::ReadCache));
        assert!(user.has_permission(&Permission::WriteCache));
        assert!(user.has_permission(&Permission::DeleteCache));
        assert!(user.has_permission(&Permission::AdminRead));
        assert!(!user.has_permission(&Permission::AdminWrite));
        assert!(!user.has_permission(&Permission::ManageUsers));
    }

    #[test]
    fn test_readonly_role_permissions() {
        let readonly = create_readonly_role();
        assert_eq!(readonly.name, "read-only");
        assert!(readonly.is_system_role);
        assert!(readonly.has_permission(&Permission::ReadCache));
        assert!(readonly.has_permission(&Permission::AdminRead));
        assert!(!readonly.has_permission(&Permission::WriteCache));
        assert!(!readonly.has_permission(&Permission::DeleteCache));
        assert!(!readonly.has_permission(&Permission::AdminWrite));
    }

    #[test]
    fn test_create_default_admin() {
        let result = create_default_admin(
            "admin".to_string(),
            "admin123".to_string(),
            "role_id_123".to_string(),
        );

        assert!(result.is_ok());
        let admin = result.unwrap();
        assert_eq!(admin.username, "admin");
        assert_eq!(admin.role_ids, vec!["role_id_123"]);
        assert!(!admin.password_hash.is_empty());
    }
}
