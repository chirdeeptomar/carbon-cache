use super::error::AuthError;
use super::models::{Role, User};
use super::repository::{RoleRepository, UserRepository};
use async_trait::async_trait;
use sled::Db;
use std::path::Path;

const USERS_TREE: &str = "users";
const USERS_BY_USERNAME_TREE: &str = "users_by_username";
const ROLES_TREE: &str = "roles";
const ROLES_BY_NAME_TREE: &str = "roles_by_name";

#[derive(Clone)]
pub struct SledUserRepository {
    db: Db,
}

impl SledUserRepository {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, AuthError> {
        let db = sled::open(path)?;
        Ok(Self { db })
    }

    fn users_tree(&self) -> Result<sled::Tree, AuthError> {
        Ok(self.db.open_tree(USERS_TREE)?)
    }

    fn users_by_username_tree(&self) -> Result<sled::Tree, AuthError> {
        Ok(self.db.open_tree(USERS_BY_USERNAME_TREE)?)
    }
}

#[async_trait]
impl UserRepository for SledUserRepository {
    async fn create(&self, user: User) -> Result<User, AuthError> {
        if self.username_exists(&user.username).await? {
            return Err(AuthError::UserAlreadyExists);
        }

        let users_tree = self.users_tree()?;
        let username_tree = self.users_by_username_tree()?;

        let user_json = serde_json::to_vec(&user)?;

        // Store user by ID
        users_tree.insert(user.id.as_bytes(), user_json.clone())?;

        // Store ID by username for lookups
        username_tree.insert(user.username.as_bytes(), user.id.as_bytes())?;

        Ok(user)
    }

    async fn find_by_username(&self, username: &str) -> Result<Option<User>, AuthError> {
        let username_tree = self.users_by_username_tree()?;
        let users_tree = self.users_tree()?;

        // First, get the user ID from username index
        if let Some(user_id) = username_tree.get(username.as_bytes())? {
            // Then get the user by ID
            if let Some(user_data) = users_tree.get(&user_id)? {
                let user: User = serde_json::from_slice(&user_data)?;
                return Ok(Some(user));
            }
        }

        Ok(None)
    }

    async fn find_by_id(&self, id: &str) -> Result<Option<User>, AuthError> {
        let users_tree = self.users_tree()?;

        if let Some(user_data) = users_tree.get(id.as_bytes())? {
            let user: User = serde_json::from_slice(&user_data)?;
            return Ok(Some(user));
        }

        Ok(None)
    }

    async fn list_all(&self) -> Result<Vec<User>, AuthError> {
        let users_tree = self.users_tree()?;
        let mut users = Vec::new();

        for item in users_tree.iter() {
            let (_, user_data) = item?;
            let user: User = serde_json::from_slice(&user_data)?;
            users.push(user);
        }

        Ok(users)
    }

    async fn update(&self, user: User) -> Result<User, AuthError> {
        let users_tree = self.users_tree()?;
        let username_tree = self.users_by_username_tree()?;

        // Check if user exists
        if !users_tree.contains_key(user.id.as_bytes())? {
            return Err(AuthError::UserNotFound);
        }

        let user_json = serde_json::to_vec(&user)?;

        // Update user by ID
        users_tree.insert(user.id.as_bytes(), user_json)?;

        // Update username index
        username_tree.insert(user.username.as_bytes(), user.id.as_bytes())?;

        Ok(user)
    }

    async fn delete(&self, id: &str) -> Result<(), AuthError> {
        let users_tree = self.users_tree()?;
        let username_tree = self.users_by_username_tree()?;

        // Get user to find username
        if let Some(user_data) = users_tree.get(id.as_bytes())? {
            let user: User = serde_json::from_slice(&user_data)?;

            // Remove from username index
            username_tree.remove(user.username.as_bytes())?;

            // Remove user by ID
            users_tree.remove(id.as_bytes())?;

            Ok(())
        } else {
            Err(AuthError::UserNotFound)
        }
    }

    async fn username_exists(&self, username: &str) -> Result<bool, AuthError> {
        let username_tree = self.users_by_username_tree()?;
        Ok(username_tree.contains_key(username.as_bytes())?)
    }
}

#[derive(Clone)]
pub struct SledRoleRepository {
    db: Db,
}

impl SledRoleRepository {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, AuthError> {
        let db = sled::open(path)?;
        Ok(Self { db })
    }

    fn roles_tree(&self) -> Result<sled::Tree, AuthError> {
        Ok(self.db.open_tree(ROLES_TREE)?)
    }

    fn roles_by_name_tree(&self) -> Result<sled::Tree, AuthError> {
        Ok(self.db.open_tree(ROLES_BY_NAME_TREE)?)
    }
}

#[async_trait]
impl RoleRepository for SledRoleRepository {
    async fn create(&self, role: Role) -> Result<Role, AuthError> {
        if self.name_exists(&role.name).await? {
            return Err(AuthError::RoleAlreadyExists);
        }

        let roles_tree = self.roles_tree()?;
        let name_tree = self.roles_by_name_tree()?;

        let role_json = serde_json::to_vec(&role)?;

        // Store role by ID
        roles_tree.insert(role.id.as_bytes(), role_json)?;

        // Store ID by name for lookups
        name_tree.insert(role.name.as_bytes(), role.id.as_bytes())?;

        Ok(role)
    }

    async fn find_by_name(&self, name: &str) -> Result<Option<Role>, AuthError> {
        let name_tree = self.roles_by_name_tree()?;
        let roles_tree = self.roles_tree()?;

        // First, get the role ID from name index
        if let Some(role_id) = name_tree.get(name.as_bytes())? {
            // Then get the role by ID
            if let Some(role_data) = roles_tree.get(&role_id)? {
                let role: Role = serde_json::from_slice(&role_data)?;
                return Ok(Some(role));
            }
        }

        Ok(None)
    }

    async fn find_by_id(&self, id: &str) -> Result<Option<Role>, AuthError> {
        let roles_tree = self.roles_tree()?;

        if let Some(role_data) = roles_tree.get(id.as_bytes())? {
            let role: Role = serde_json::from_slice(&role_data)?;
            return Ok(Some(role));
        }

        Ok(None)
    }

    async fn find_by_ids(&self, ids: &[String]) -> Result<Vec<Role>, AuthError> {
        let roles_tree = self.roles_tree()?;
        let mut roles = Vec::new();

        for id in ids {
            if let Some(role_data) = roles_tree.get(id.as_bytes())? {
                let role: Role = serde_json::from_slice(&role_data)?;
                roles.push(role);
            }
        }

        Ok(roles)
    }

    async fn list_all(&self) -> Result<Vec<Role>, AuthError> {
        let roles_tree = self.roles_tree()?;
        let mut roles = Vec::new();

        for item in roles_tree.iter() {
            let (_, role_data) = item?;
            let role: Role = serde_json::from_slice(&role_data)?;
            roles.push(role);
        }

        Ok(roles)
    }

    async fn update(&self, role: Role) -> Result<Role, AuthError> {
        let roles_tree = self.roles_tree()?;
        let name_tree = self.roles_by_name_tree()?;

        // Check if role exists
        if !roles_tree.contains_key(role.id.as_bytes())? {
            return Err(AuthError::RoleNotFound);
        }

        let role_json = serde_json::to_vec(&role)?;

        // Update role by ID
        roles_tree.insert(role.id.as_bytes(), role_json)?;

        // Update name index
        name_tree.insert(role.name.as_bytes(), role.id.as_bytes())?;

        Ok(role)
    }

    async fn delete(&self, id: &str) -> Result<(), AuthError> {
        let roles_tree = self.roles_tree()?;
        let name_tree = self.roles_by_name_tree()?;

        // Get role to find name and check if system role
        if let Some(role_data) = roles_tree.get(id.as_bytes())? {
            let role: Role = serde_json::from_slice(&role_data)?;

            if role.is_system_role {
                return Err(AuthError::CannotDeleteSystemRole);
            }

            // Remove from name index
            name_tree.remove(role.name.as_bytes())?;

            // Remove role by ID
            roles_tree.remove(id.as_bytes())?;

            Ok(())
        } else {
            Err(AuthError::RoleNotFound)
        }
    }

    async fn name_exists(&self, name: &str) -> Result<bool, AuthError> {
        let name_tree = self.roles_by_name_tree()?;
        Ok(name_tree.contains_key(name.as_bytes())?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_user_repository() {
        let temp_dir = TempDir::new().unwrap();
        let repo = SledUserRepository::new(temp_dir.path().join("users.sled")).unwrap();

        let user = User::new(
            "testuser".to_string(),
            "hash123".to_string(),
            vec!["role1".to_string()],
        );

        // Create
        let created = repo.create(user.clone()).await.unwrap();
        assert_eq!(created.username, "testuser");

        // Find by username
        let found = repo.find_by_username("testuser").await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().username, "testuser");

        // Find by ID
        let found_by_id = repo.find_by_id(&created.id).await.unwrap();
        assert!(found_by_id.is_some());

        // List all
        let all_users = repo.list_all().await.unwrap();
        assert_eq!(all_users.len(), 1);

        // Delete
        repo.delete(&created.id).await.unwrap();
        let not_found = repo.find_by_username("testuser").await.unwrap();
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn test_role_repository() {
        let temp_dir = TempDir::new().unwrap();
        let repo = SledRoleRepository::new(temp_dir.path().join("roles.sled")).unwrap();

        let mut permissions = HashSet::new();
        permissions.insert(super::super::models::Permission::ReadCache);

        let role = Role::new("admin".to_string(), permissions, false);

        // Create
        let created = repo.create(role.clone()).await.unwrap();
        assert_eq!(created.name, "admin");

        // Find by name
        let found = repo.find_by_name("admin").await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "admin");

        // Cannot delete system role
        let system_role = Role::new("system".to_string(), HashSet::new(), true);
        let created_system = repo.create(system_role).await.unwrap();

        let delete_result = repo.delete(&created_system.id).await;
        assert!(matches!(
            delete_result,
            Err(AuthError::CannotDeleteSystemRole)
        ));

        // Can delete non-system role
        repo.delete(&created.id).await.unwrap();
        let not_found = repo.find_by_name("admin").await.unwrap();
        assert!(not_found.is_none());
    }
}
