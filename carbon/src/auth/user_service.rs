use super::error::AuthError;
use super::models::User;
use super::password::{hash_password, verify_password};
use super::repository::{RoleRepository, UserRepository};
use chrono::Utc;
use std::sync::Arc;

pub struct UserService {
    user_repo: Arc<dyn UserRepository>,
    role_repo: Arc<dyn RoleRepository>,
}

impl UserService {
    pub fn new(user_repo: Arc<dyn UserRepository>, role_repo: Arc<dyn RoleRepository>) -> Self {
        Self {
            user_repo,
            role_repo,
        }
    }

    /// Create a new user
    pub async fn create_user(
        &self,
        username: String,
        password: String,
        role_ids: Vec<String>,
    ) -> Result<User, AuthError> {
        // Check if username already exists
        if self.user_repo.username_exists(&username).await? {
            return Err(AuthError::UserAlreadyExists);
        }

        // Validate that all role IDs exist
        self.validate_role_ids(&role_ids).await?;

        // Hash the password
        let password_hash = hash_password(&password)?;

        // Create user
        let user = User::new(username, password_hash, role_ids);

        self.user_repo.create(user).await
    }

    /// Get a user by username
    pub async fn get_user(&self, username: &str) -> Result<User, AuthError> {
        self.user_repo
            .find_by_username(username)
            .await?
            .ok_or(AuthError::UserNotFound)
    }

    /// Get a user by ID
    pub async fn get_user_by_id(&self, id: &str) -> Result<User, AuthError> {
        self.user_repo
            .find_by_id(id)
            .await?
            .ok_or(AuthError::UserNotFound)
    }

    /// List all users
    pub async fn list_users(&self) -> Result<Vec<User>, AuthError> {
        self.user_repo.list_all().await
    }

    /// Update user's roles
    pub async fn assign_roles(
        &self,
        user_id: &str,
        role_ids: Vec<String>,
    ) -> Result<User, AuthError> {
        // Validate that all role IDs exist
        self.validate_role_ids(&role_ids).await?;

        // Get the user
        let mut user = self.get_user_by_id(user_id).await?;

        // Update roles and timestamp
        user.role_ids = role_ids;
        user.updated_at = Utc::now();

        self.user_repo.update(user).await
    }

    /// Change user's password
    pub async fn change_password(
        &self,
        user_id: &str,
        old_password: &str,
        new_password: &str,
    ) -> Result<User, AuthError> {
        // Get the user
        let mut user = self.get_user_by_id(user_id).await?;

        // Verify old password
        let is_valid = verify_password(old_password, &user.password_hash)?;
        if !is_valid {
            return Err(AuthError::InvalidCredentials);
        }

        // Hash new password
        let new_hash = hash_password(new_password)?;

        // Update password and timestamp
        user.password_hash = new_hash;
        user.updated_at = Utc::now();

        self.user_repo.update(user).await
    }

    /// Reset user's password (admin operation, no old password required)
    pub async fn reset_password(
        &self,
        user_id: &str,
        new_password: String,
    ) -> Result<User, AuthError> {
        // Get the user
        let mut user = self.get_user_by_id(user_id).await?;

        // Hash new password
        let new_hash = hash_password(&new_password)?;

        // Update password and timestamp
        user.password_hash = new_hash;
        user.updated_at = Utc::now();

        self.user_repo.update(user).await
    }

    /// Delete a user
    pub async fn delete_user(
        &self,
        user_id: &str,
        requesting_user_id: &str,
    ) -> Result<(), AuthError> {
        // Prevent users from deleting themselves
        if user_id == requesting_user_id {
            return Err(AuthError::CannotDeleteSelf);
        }

        self.user_repo.delete(user_id).await
    }

    /// Validate that all role IDs exist
    async fn validate_role_ids(&self, role_ids: &[String]) -> Result<(), AuthError> {
        if role_ids.is_empty() {
            return Err(AuthError::InvalidRoleAssignment);
        }

        let roles = self.role_repo.find_by_ids(role_ids).await?;

        if roles.len() != role_ids.len() {
            return Err(AuthError::RoleNotFound);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::defaults::create_user_role;
    use crate::auth::sled_repository::{SledRoleRepository, SledUserRepository};
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_create_user() {
        let temp_dir = TempDir::new().unwrap();
        let user_repo =
            Arc::new(SledUserRepository::new(temp_dir.path().join("users.sled")).unwrap())
                as Arc<dyn UserRepository>;
        let role_repo =
            Arc::new(SledRoleRepository::new(temp_dir.path().join("roles.sled")).unwrap())
                as Arc<dyn RoleRepository>;

        // Create a role first
        let role = create_user_role();
        let created_role = role_repo.create(role).await.unwrap();

        let user_service = UserService::new(user_repo, role_repo);

        // Create user
        let result = user_service
            .create_user(
                "testuser".to_string(),
                "testpass123".to_string(),
                vec![created_role.id.clone()],
            )
            .await;

        assert!(result.is_ok());
        let user = result.unwrap();
        assert_eq!(user.username, "testuser");
        assert_eq!(user.role_ids, vec![created_role.id]);
    }

    #[tokio::test]
    async fn test_change_password() {
        let temp_dir = TempDir::new().unwrap();
        let user_repo =
            Arc::new(SledUserRepository::new(temp_dir.path().join("users.sled")).unwrap())
                as Arc<dyn UserRepository>;
        let role_repo =
            Arc::new(SledRoleRepository::new(temp_dir.path().join("roles.sled")).unwrap())
                as Arc<dyn RoleRepository>;

        let role = create_user_role();
        let created_role = role_repo.create(role).await.unwrap();

        let user_service = UserService::new(user_repo, role_repo);

        // Create user
        let user = user_service
            .create_user(
                "testuser".to_string(),
                "oldpass123".to_string(),
                vec![created_role.id],
            )
            .await
            .unwrap();

        // Change password
        let result = user_service
            .change_password(&user.id, "oldpass123", "newpass123")
            .await;

        assert!(result.is_ok());

        // Verify new password works
        let updated_user = result.unwrap();
        assert!(verify_password("newpass123", &updated_user.password_hash).unwrap());
    }

    #[tokio::test]
    async fn test_cannot_delete_self() {
        let temp_dir = TempDir::new().unwrap();
        let user_repo =
            Arc::new(SledUserRepository::new(temp_dir.path().join("users.sled")).unwrap())
                as Arc<dyn UserRepository>;
        let role_repo =
            Arc::new(SledRoleRepository::new(temp_dir.path().join("roles.sled")).unwrap())
                as Arc<dyn RoleRepository>;

        let role = create_user_role();
        let created_role = role_repo.create(role).await.unwrap();

        let user_service = UserService::new(user_repo, role_repo);

        let user = user_service
            .create_user(
                "testuser".to_string(),
                "oldpass123".to_string(),
                vec![created_role.id],
            )
            .await
            .unwrap();

        // Try to delete self
        let result = user_service.delete_user(&user.id, &user.id).await;
        assert!(matches!(result, Err(AuthError::CannotDeleteSelf)));
    }
}
