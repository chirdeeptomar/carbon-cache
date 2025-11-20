use super::error::AuthError;
use super::models::{Permission, Role};
use super::repository::RoleRepository;
use std::collections::HashSet;
use std::sync::Arc;

pub struct RoleService {
    role_repo: Arc<dyn RoleRepository>,
}

impl RoleService {
    pub fn new(role_repo: Arc<dyn RoleRepository>) -> Self {
        Self { role_repo }
    }

    /// Create a new custom role
    pub async fn create_role(
        &self,
        name: String,
        permissions: HashSet<Permission>,
    ) -> Result<Role, AuthError> {
        // Check if role name already exists
        if self.role_repo.name_exists(&name).await? {
            return Err(AuthError::RoleAlreadyExists);
        }

        // Create role (custom roles are not system roles)
        let role = Role::new(name, permissions, false);

        self.role_repo.create(role).await
    }

    /// Get a role by name
    pub async fn get_role(&self, name: &str) -> Result<Role, AuthError> {
        self.role_repo
            .find_by_name(name)
            .await?
            .ok_or(AuthError::RoleNotFound)
    }

    /// Get a role by ID
    pub async fn get_role_by_id(&self, id: &str) -> Result<Role, AuthError> {
        self.role_repo
            .find_by_id(id)
            .await?
            .ok_or(AuthError::RoleNotFound)
    }

    /// List all roles
    pub async fn list_roles(&self) -> Result<Vec<Role>, AuthError> {
        self.role_repo.list_all().await
    }

    /// Update a role's permissions
    pub async fn update_role(
        &self,
        role_id: &str,
        permissions: HashSet<Permission>,
    ) -> Result<Role, AuthError> {
        // Get the role
        let mut role = self.get_role_by_id(role_id).await?;

        // Prevent modification of system roles
        if role.is_system_role {
            return Err(AuthError::CannotDeleteSystemRole); // Reusing error for modification
        }

        // Update permissions
        role.permissions = permissions;

        self.role_repo.update(role).await
    }

    /// Delete a role
    pub async fn delete_role(&self, role_id: &str) -> Result<(), AuthError> {
        // The repository will check if it's a system role and prevent deletion
        self.role_repo.delete(role_id).await
    }

    /// Initialize default system roles if they don't exist
    pub async fn initialize_default_roles(&self) -> Result<Vec<Role>, AuthError> {
        use super::defaults::create_default_roles;

        let default_roles = create_default_roles();
        let mut created_roles = Vec::new();

        for role in default_roles {
            // Check if role already exists
            if self.role_repo.name_exists(&role.name).await? {
                // If exists, get it
                let existing = self.get_role(&role.name).await?;
                created_roles.push(existing);
            } else {
                // Create new role
                let created = self.role_repo.create(role).await?;
                created_roles.push(created);
            }
        }

        Ok(created_roles)
    }

    /// Get role by name, returning None if not found (helper method)
    pub async fn find_role_by_name(&self, name: &str) -> Result<Option<Role>, AuthError> {
        self.role_repo.find_by_name(name).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::models::Permission;
    use crate::auth::sled_repository::SledRoleRepository;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_create_role() {
        let temp_dir = TempDir::new().unwrap();
        let role_repo = Arc::new(
            SledRoleRepository::new(temp_dir.path().join("roles.sled")).unwrap(),
        ) as Arc<dyn RoleRepository>;

        let role_service = RoleService::new(role_repo);

        let mut permissions = HashSet::new();
        permissions.insert(Permission::ReadCache);
        permissions.insert(Permission::WriteCache);

        let result = role_service
            .create_role("custom_role".to_string(), permissions)
            .await;

        assert!(result.is_ok());
        let role = result.unwrap();
        assert_eq!(role.name, "custom_role");
        assert!(!role.is_system_role);
    }

    #[tokio::test]
    async fn test_update_role() {
        let temp_dir = TempDir::new().unwrap();
        let role_repo = Arc::new(
            SledRoleRepository::new(temp_dir.path().join("roles.sled")).unwrap(),
        ) as Arc<dyn RoleRepository>;

        let role_service = RoleService::new(role_repo);

        // Create a role
        let mut permissions = HashSet::new();
        permissions.insert(Permission::ReadCache);

        let role = role_service
            .create_role("custom_role".to_string(), permissions)
            .await
            .unwrap();

        // Update permissions
        let mut new_permissions = HashSet::new();
        new_permissions.insert(Permission::ReadCache);
        new_permissions.insert(Permission::WriteCache);

        let updated = role_service
            .update_role(&role.id, new_permissions)
            .await
            .unwrap();

        assert!(updated.has_permission(&Permission::ReadCache));
        assert!(updated.has_permission(&Permission::WriteCache));
    }

    #[tokio::test]
    async fn test_initialize_default_roles() {
        let temp_dir = TempDir::new().unwrap();
        let role_repo = Arc::new(
            SledRoleRepository::new(temp_dir.path().join("roles.sled")).unwrap(),
        ) as Arc<dyn RoleRepository>;

        let role_service = RoleService::new(role_repo);

        let roles = role_service.initialize_default_roles().await.unwrap();

        assert_eq!(roles.len(), 3);
        assert!(roles.iter().any(|r| r.name == "admin"));
        assert!(roles.iter().any(|r| r.name == "user"));
        assert!(roles.iter().any(|r| r.name == "read-only"));

        // All should be system roles
        assert!(roles.iter().all(|r| r.is_system_role));
    }

    #[tokio::test]
    async fn test_cannot_update_system_role() {
        let temp_dir = TempDir::new().unwrap();
        let role_repo = Arc::new(
            SledRoleRepository::new(temp_dir.path().join("roles.sled")).unwrap(),
        ) as Arc<dyn RoleRepository>;

        let role_service = RoleService::new(role_repo);

        // Initialize default roles
        let roles = role_service.initialize_default_roles().await.unwrap();
        let admin_role = roles.iter().find(|r| r.name == "admin").unwrap();

        // Try to update system role
        let result = role_service
            .update_role(&admin_role.id, HashSet::new())
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_cannot_delete_system_role() {
        let temp_dir = TempDir::new().unwrap();
        let role_repo = Arc::new(
            SledRoleRepository::new(temp_dir.path().join("roles.sled")).unwrap(),
        ) as Arc<dyn RoleRepository>;

        let role_service = RoleService::new(role_repo);

        // Initialize default roles
        let roles = role_service.initialize_default_roles().await.unwrap();
        let admin_role = roles.iter().find(|r| r.name == "admin").unwrap();

        // Try to delete system role
        let result = role_service.delete_role(&admin_role.id).await;

        assert!(matches!(result, Err(AuthError::CannotDeleteSystemRole)));
    }
}
