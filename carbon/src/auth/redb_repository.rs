use super::error::AuthError;
use super::models::{Role, User};
use super::repository::{RoleRepository, UserRepository};
use async_trait::async_trait;
use redb::{Database, ReadableTable, TableDefinition};
use std::path::Path;
use std::sync::Arc;

const USERS: TableDefinition<&str, &[u8]> = TableDefinition::new("users");
const USERS_BY_USERNAME: TableDefinition<&str, &str> = TableDefinition::new("users_by_username");
const ROLES: TableDefinition<&str, &[u8]> = TableDefinition::new("roles");
const ROLES_BY_NAME: TableDefinition<&str, &str> = TableDefinition::new("roles_by_name");

#[derive(Clone)]
pub struct RedbUserRepository {
    db: Arc<Database>,
}

impl RedbUserRepository {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, AuthError> {
        let db = Database::create(path).map_err(|e| AuthError::StorageError(e.to_string()))?;
        let db = Arc::new(db);
        let write_txn = db.begin_write()?;
        {
            write_txn.open_table(USERS)?;
            write_txn.open_table(USERS_BY_USERNAME)?;
        }
        write_txn.commit()?;
        Ok(Self { db })
    }
}

#[async_trait]
impl UserRepository for RedbUserRepository {
    async fn create(&self, user: User) -> Result<User, AuthError> {
        if self.username_exists(&user.username).await? {
            return Err(AuthError::UserAlreadyExists);
        }
        let user_json = serde_json::to_vec(&user)?;
        let write_txn = self.db.begin_write()?;
        {
            let mut users = write_txn.open_table(USERS)?;
            let mut by_username = write_txn.open_table(USERS_BY_USERNAME)?;
            users.insert(user.id.as_str(), user_json.as_slice())?;
            by_username.insert(user.username.as_str(), user.id.as_str())?;
        }
        write_txn.commit()?;
        Ok(user)
    }

    async fn find_by_username(&self, username: &str) -> Result<Option<User>, AuthError> {
        let read_txn = self.db.begin_read().map_err(|e| AuthError::StorageError(e.to_string()))?;
        let by_username = read_txn.open_table(USERS_BY_USERNAME).map_err(|e| AuthError::StorageError(e.to_string()))?;
        let users = read_txn.open_table(USERS).map_err(|e| AuthError::StorageError(e.to_string()))?;
        if let Some(id_guard) = by_username.get(username).map_err(|e| AuthError::StorageError(e.to_string()))? {
            let id = id_guard.value().to_owned();
            if let Some(data_guard) = users.get(id.as_str()).map_err(|e| AuthError::StorageError(e.to_string()))? {
                let user: User = serde_json::from_slice(data_guard.value())?;
                return Ok(Some(user));
            }
        }
        Ok(None)
    }

    async fn find_by_id(&self, id: &str) -> Result<Option<User>, AuthError> {
        let read_txn = self.db.begin_read().map_err(|e| AuthError::StorageError(e.to_string()))?;
        let users = read_txn.open_table(USERS).map_err(|e| AuthError::StorageError(e.to_string()))?;
        if let Some(data_guard) = users.get(id).map_err(|e| AuthError::StorageError(e.to_string()))? {
            let user: User = serde_json::from_slice(data_guard.value())?;
            return Ok(Some(user));
        }
        Ok(None)
    }

    async fn list_all(&self) -> Result<Vec<User>, AuthError> {
        let read_txn = self.db.begin_read().map_err(|e| AuthError::StorageError(e.to_string()))?;
        let users = read_txn.open_table(USERS).map_err(|e| AuthError::StorageError(e.to_string()))?;
        let mut result = Vec::new();
        for entry in users.iter().map_err(|e| AuthError::StorageError(e.to_string()))? {
            let (_, v) = entry.map_err(|e| AuthError::StorageError(e.to_string()))?;
            let user: User = serde_json::from_slice(v.value())?;
            result.push(user);
        }
        Ok(result)
    }

    async fn update(&self, user: User) -> Result<User, AuthError> {
        let exists = self.find_by_id(&user.id).await?.is_some();
        if !exists {
            return Err(AuthError::UserNotFound);
        }
        let user_json = serde_json::to_vec(&user)?;
        let write_txn = self.db.begin_write()?;
        {
            let mut users = write_txn.open_table(USERS)?;
            let mut by_username = write_txn.open_table(USERS_BY_USERNAME)?;
            users.insert(user.id.as_str(), user_json.as_slice())?;
            by_username.insert(user.username.as_str(), user.id.as_str())?;
        }
        write_txn.commit()?;
        Ok(user)
    }

    async fn delete(&self, id: &str) -> Result<(), AuthError> {
        let user = self.find_by_id(id).await?.ok_or(AuthError::UserNotFound)?;
        let write_txn = self.db.begin_write()?;
        {
            let mut users = write_txn.open_table(USERS)?;
            let mut by_username = write_txn.open_table(USERS_BY_USERNAME)?;
            users.remove(id)?;
            by_username.remove(user.username.as_str())?;
        }
        write_txn.commit()?;
        Ok(())
    }

    async fn username_exists(&self, username: &str) -> Result<bool, AuthError> {
        let read_txn = self.db.begin_read().map_err(|e| AuthError::StorageError(e.to_string()))?;
        let by_username = read_txn.open_table(USERS_BY_USERNAME).map_err(|e| AuthError::StorageError(e.to_string()))?;
        Ok(by_username.get(username).map_err(|e| AuthError::StorageError(e.to_string()))?.is_some())
    }
}

#[derive(Clone)]
pub struct RedbRoleRepository {
    db: Arc<Database>,
}

impl RedbRoleRepository {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, AuthError> {
        let db = Database::create(path).map_err(|e| AuthError::StorageError(e.to_string()))?;
        let db = Arc::new(db);
        let write_txn = db.begin_write()?;
        {
            write_txn.open_table(ROLES)?;
            write_txn.open_table(ROLES_BY_NAME)?;
        }
        write_txn.commit()?;
        Ok(Self { db })
    }
}

#[async_trait]
impl RoleRepository for RedbRoleRepository {
    async fn create(&self, role: Role) -> Result<Role, AuthError> {
        if self.name_exists(&role.name).await? {
            return Err(AuthError::RoleAlreadyExists);
        }
        let role_json = serde_json::to_vec(&role)?;
        let write_txn = self.db.begin_write()?;
        {
            let mut roles = write_txn.open_table(ROLES)?;
            let mut by_name = write_txn.open_table(ROLES_BY_NAME)?;
            roles.insert(role.id.as_str(), role_json.as_slice())?;
            by_name.insert(role.name.as_str(), role.id.as_str())?;
        }
        write_txn.commit()?;
        Ok(role)
    }

    async fn find_by_name(&self, name: &str) -> Result<Option<Role>, AuthError> {
        let read_txn = self.db.begin_read().map_err(|e| AuthError::StorageError(e.to_string()))?;
        let by_name = read_txn.open_table(ROLES_BY_NAME).map_err(|e| AuthError::StorageError(e.to_string()))?;
        let roles = read_txn.open_table(ROLES).map_err(|e| AuthError::StorageError(e.to_string()))?;
        if let Some(id_guard) = by_name.get(name).map_err(|e| AuthError::StorageError(e.to_string()))? {
            let id = id_guard.value().to_owned();
            if let Some(data_guard) = roles.get(id.as_str()).map_err(|e| AuthError::StorageError(e.to_string()))? {
                let role: Role = serde_json::from_slice(data_guard.value())?;
                return Ok(Some(role));
            }
        }
        Ok(None)
    }

    async fn find_by_id(&self, id: &str) -> Result<Option<Role>, AuthError> {
        let read_txn = self.db.begin_read().map_err(|e| AuthError::StorageError(e.to_string()))?;
        let roles = read_txn.open_table(ROLES).map_err(|e| AuthError::StorageError(e.to_string()))?;
        if let Some(data_guard) = roles.get(id).map_err(|e| AuthError::StorageError(e.to_string()))? {
            let role: Role = serde_json::from_slice(data_guard.value())?;
            return Ok(Some(role));
        }
        Ok(None)
    }

    async fn find_by_ids(&self, ids: &[String]) -> Result<Vec<Role>, AuthError> {
        let read_txn = self.db.begin_read().map_err(|e| AuthError::StorageError(e.to_string()))?;
        let roles = read_txn.open_table(ROLES).map_err(|e| AuthError::StorageError(e.to_string()))?;
        let mut result = Vec::new();
        for id in ids {
            if let Some(data_guard) = roles.get(id.as_str()).map_err(|e| AuthError::StorageError(e.to_string()))? {
                let role: Role = serde_json::from_slice(data_guard.value())?;
                result.push(role);
            }
        }
        Ok(result)
    }

    async fn list_all(&self) -> Result<Vec<Role>, AuthError> {
        let read_txn = self.db.begin_read().map_err(|e| AuthError::StorageError(e.to_string()))?;
        let roles = read_txn.open_table(ROLES).map_err(|e| AuthError::StorageError(e.to_string()))?;
        let mut result = Vec::new();
        for entry in roles.iter().map_err(|e| AuthError::StorageError(e.to_string()))? {
            let (_, v) = entry.map_err(|e| AuthError::StorageError(e.to_string()))?;
            let role: Role = serde_json::from_slice(v.value())?;
            result.push(role);
        }
        Ok(result)
    }

    async fn update(&self, role: Role) -> Result<Role, AuthError> {
        let exists = self.find_by_id(&role.id).await?.is_some();
        if !exists {
            return Err(AuthError::RoleNotFound);
        }
        let role_json = serde_json::to_vec(&role)?;
        let write_txn = self.db.begin_write()?;
        {
            let mut roles = write_txn.open_table(ROLES)?;
            let mut by_name = write_txn.open_table(ROLES_BY_NAME)?;
            roles.insert(role.id.as_str(), role_json.as_slice())?;
            by_name.insert(role.name.as_str(), role.id.as_str())?;
        }
        write_txn.commit()?;
        Ok(role)
    }

    async fn delete(&self, id: &str) -> Result<(), AuthError> {
        let role = self.find_by_id(id).await?.ok_or(AuthError::RoleNotFound)?;
        if role.is_system_role {
            return Err(AuthError::CannotDeleteSystemRole);
        }
        let write_txn = self.db.begin_write()?;
        {
            let mut roles = write_txn.open_table(ROLES)?;
            let mut by_name = write_txn.open_table(ROLES_BY_NAME)?;
            roles.remove(id)?;
            by_name.remove(role.name.as_str())?;
        }
        write_txn.commit()?;
        Ok(())
    }

    async fn name_exists(&self, name: &str) -> Result<bool, AuthError> {
        let read_txn = self.db.begin_read().map_err(|e| AuthError::StorageError(e.to_string()))?;
        let by_name = read_txn.open_table(ROLES_BY_NAME).map_err(|e| AuthError::StorageError(e.to_string()))?;
        Ok(by_name.get(name).map_err(|e| AuthError::StorageError(e.to_string()))?.is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use tempfile::TempDir;

    fn make_user_repo(dir: &TempDir) -> RedbUserRepository {
        RedbUserRepository::new(dir.path().join("auth.redb")).unwrap()
    }

    fn make_role_repo(dir: &TempDir) -> RedbRoleRepository {
        RedbRoleRepository::new(dir.path().join("auth.redb")).unwrap()
    }

    #[tokio::test]
    async fn test_user_create_find_delete() {
        let dir = TempDir::new().unwrap();
        let repo = make_user_repo(&dir);

        let user = User::new("alice".to_string(), "hash".to_string(), vec![]);
        let created = repo.create(user).await.unwrap();
        assert_eq!(created.username, "alice");

        let by_name = repo.find_by_username("alice").await.unwrap();
        assert!(by_name.is_some());

        let by_id = repo.find_by_id(&created.id).await.unwrap();
        assert!(by_id.is_some());

        let all = repo.list_all().await.unwrap();
        assert_eq!(all.len(), 1);

        repo.delete(&created.id).await.unwrap();
        assert!(repo.find_by_username("alice").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_user_duplicate_rejected() {
        let dir = TempDir::new().unwrap();
        let repo = make_user_repo(&dir);
        let user = User::new("bob".to_string(), "hash".to_string(), vec![]);
        repo.create(user.clone()).await.unwrap();
        let result = repo.create(user).await;
        assert!(matches!(result, Err(AuthError::UserAlreadyExists)));
    }

    #[tokio::test]
    async fn test_user_update() {
        let dir = TempDir::new().unwrap();
        let repo = make_user_repo(&dir);
        let user = User::new("carol".to_string(), "hash".to_string(), vec![]);
        let mut created = repo.create(user).await.unwrap();
        created.password_hash = "newhash".to_string();
        let updated = repo.update(created.clone()).await.unwrap();
        assert_eq!(updated.password_hash, "newhash");
    }

    #[tokio::test]
    async fn test_role_create_find_delete() {
        let dir = TempDir::new().unwrap();
        let repo = make_role_repo(&dir);

        let mut perms = HashSet::new();
        perms.insert(super::super::models::Permission::ReadCache);
        let role = Role::new("editor".to_string(), perms, false);

        let created = repo.create(role).await.unwrap();
        assert_eq!(created.name, "editor");

        let by_name = repo.find_by_name("editor").await.unwrap();
        assert!(by_name.is_some());

        repo.delete(&created.id).await.unwrap();
        assert!(repo.find_by_name("editor").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_cannot_delete_system_role() {
        let dir = TempDir::new().unwrap();
        let repo = make_role_repo(&dir);
        let role = Role::new("system".to_string(), HashSet::new(), true);
        let created = repo.create(role).await.unwrap();
        let result = repo.delete(&created.id).await;
        assert!(matches!(result, Err(AuthError::CannotDeleteSystemRole)));
    }
}
