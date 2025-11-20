use super::error::AuthError;
use super::models::{Permission, User};
use super::password::verify_password;
use super::repository::{RoleRepository, UserRepository};
use std::sync::Arc;

pub struct AuthService {
    user_repo: Arc<dyn UserRepository>,
    role_repo: Arc<dyn RoleRepository>,
}

impl AuthService {
    pub fn new(
        user_repo: Arc<dyn UserRepository>,
        role_repo: Arc<dyn RoleRepository>,
    ) -> Self {
        Self {
            user_repo,
            role_repo,
        }
    }

    /// Authenticate a user by username and password
    pub async fn authenticate(&self, username: &str, password: &str) -> Result<User, AuthError> {
        // Find user by username
        let user = self
            .user_repo
            .find_by_username(username)
            .await?
            .ok_or(AuthError::InvalidCredentials)?;

        // Verify password
        let is_valid = verify_password(password, &user.password_hash)?;

        if !is_valid {
            return Err(AuthError::InvalidCredentials);
        }

        Ok(user)
    }

    /// Check if a user has a specific permission
    pub async fn authorize(&self, user: &User, permission: Permission) -> Result<(), AuthError> {
        // Load all roles for the user
        let roles = self.role_repo.find_by_ids(&user.role_ids).await?;

        // Check if any role has the required permission
        for role in roles {
            if role.has_permission(&permission) {
                return Ok(());
            }
        }

        Err(AuthError::PermissionDenied)
    }

    /// Check if a user has any of the specified permissions
    pub async fn has_any_permission(
        &self,
        user: &User,
        permissions: &[Permission],
    ) -> Result<(), AuthError> {
        // Load all roles for the user
        let roles = self.role_repo.find_by_ids(&user.role_ids).await?;

        // Check if any role has any of the required permissions
        for role in roles {
            if role.has_any_permission(permissions) {
                return Ok(());
            }
        }

        Err(AuthError::PermissionDenied)
    }

    /// Check if a user has all of the specified permissions
    pub async fn has_all_permissions(
        &self,
        user: &User,
        permissions: &[Permission],
    ) -> Result<(), AuthError> {
        // Load all roles for the user
        let roles = self.role_repo.find_by_ids(&user.role_ids).await?;

        // Collect all permissions from all user roles
        let mut user_permissions = std::collections::HashSet::new();
        for role in roles {
            user_permissions.extend(role.permissions.iter().cloned());
        }

        // Check if user has all required permissions
        for permission in permissions {
            if !user_permissions.contains(permission) {
                return Err(AuthError::PermissionDenied);
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::models::Role;
    use crate::auth::password::hash_password;
    use crate::auth::sled_repository::{SledRoleRepository, SledUserRepository};
    use std::collections::HashSet;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_authenticate_success() {
        let temp_dir = TempDir::new().unwrap();
        let user_repo = Arc::new(
            SledUserRepository::new(temp_dir.path().join("users.sled")).unwrap(),
        ) as Arc<dyn UserRepository>;
        let role_repo = Arc::new(
            SledRoleRepository::new(temp_dir.path().join("roles.sled")).unwrap(),
        ) as Arc<dyn RoleRepository>;

        let auth_service = AuthService::new(user_repo.clone(), role_repo);

        // Create a test user
        let password = "testpass123";
        let password_hash = hash_password(password).unwrap();
        let user = User::new(
            "testuser".to_string(),
            password_hash,
            vec!["role1".to_string()],
        );
        user_repo.create(user).await.unwrap();

        // Test authentication
        let result = auth_service.authenticate("testuser", password).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_authenticate_invalid_password() {
        let temp_dir = TempDir::new().unwrap();
        let user_repo = Arc::new(
            SledUserRepository::new(temp_dir.path().join("users.sled")).unwrap(),
        ) as Arc<dyn UserRepository>;
        let role_repo = Arc::new(
            SledRoleRepository::new(temp_dir.path().join("roles.sled")).unwrap(),
        ) as Arc<dyn RoleRepository>;

        let auth_service = AuthService::new(user_repo.clone(), role_repo);

        // Create a test user
        let password_hash = hash_password("testpass123").unwrap();
        let user = User::new(
            "testuser".to_string(),
            password_hash,
            vec!["role1".to_string()],
        );
        user_repo.create(user).await.unwrap();

        // Test authentication with wrong password
        let result = auth_service.authenticate("testuser", "wrongpass").await;
        assert!(matches!(result, Err(AuthError::InvalidCredentials)));
    }

    #[tokio::test]
    async fn test_authorize_with_permission() {
        let temp_dir = TempDir::new().unwrap();
        let user_repo = Arc::new(
            SledUserRepository::new(temp_dir.path().join("users.sled")).unwrap(),
        ) as Arc<dyn UserRepository>;
        let role_repo = Arc::new(
            SledRoleRepository::new(temp_dir.path().join("roles.sled")).unwrap(),
        ) as Arc<dyn RoleRepository>;

        let auth_service = AuthService::new(user_repo, role_repo.clone());

        // Create a role with ReadCache permission
        let mut permissions = HashSet::new();
        permissions.insert(Permission::ReadCache);
        let role = Role::new("reader".to_string(), permissions, false);
        let created_role = role_repo.create(role).await.unwrap();

        // Create a user with this role
        let user = User::new(
            "testuser".to_string(),
            "hash".to_string(),
            vec![created_role.id.clone()],
        );

        // Test authorization
        let result = auth_service.authorize(&user, Permission::ReadCache).await;
        assert!(result.is_ok());

        let result = auth_service.authorize(&user, Permission::WriteCache).await;
        assert!(matches!(result, Err(AuthError::PermissionDenied)));
    }
}
