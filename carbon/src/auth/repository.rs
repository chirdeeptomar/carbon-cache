use super::error::AuthError;
use super::models::{Role, User};
use async_trait::async_trait;

#[async_trait]
pub trait UserRepository: Send + Sync {
    /// Create a new user
    async fn create(&self, user: User) -> Result<User, AuthError>;

    /// Find a user by username
    async fn find_by_username(&self, username: &str) -> Result<Option<User>, AuthError>;

    /// Find a user by ID
    async fn find_by_id(&self, id: &str) -> Result<Option<User>, AuthError>;

    /// List all users
    async fn list_all(&self) -> Result<Vec<User>, AuthError>;

    /// Update a user
    async fn update(&self, user: User) -> Result<User, AuthError>;

    /// Delete a user by ID
    async fn delete(&self, id: &str) -> Result<(), AuthError>;

    /// Check if a username exists
    async fn username_exists(&self, username: &str) -> Result<bool, AuthError>;
}

#[async_trait]
pub trait RoleRepository: Send + Sync {
    /// Create a new role
    async fn create(&self, role: Role) -> Result<Role, AuthError>;

    /// Find a role by name
    async fn find_by_name(&self, name: &str) -> Result<Option<Role>, AuthError>;

    /// Find a role by ID
    async fn find_by_id(&self, id: &str) -> Result<Option<Role>, AuthError>;

    /// Find multiple roles by IDs
    async fn find_by_ids(&self, ids: &[String]) -> Result<Vec<Role>, AuthError>;

    /// List all roles
    async fn list_all(&self) -> Result<Vec<Role>, AuthError>;

    /// Update a role
    async fn update(&self, role: Role) -> Result<Role, AuthError>;

    /// Delete a role by ID
    async fn delete(&self, id: &str) -> Result<(), AuthError>;

    /// Check if a role name exists
    async fn name_exists(&self, name: &str) -> Result<bool, AuthError>;
}
