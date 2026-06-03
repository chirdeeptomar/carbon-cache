# Sled → redb Migration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the sled embedded database with redb across the auth and persistence storage layers, renaming all `Sled*` structs/files to `Redb*`.

**Architecture:** Swap sled's implicit per-op writes for redb's explicit typed table definitions and write transactions. The two auth repositories share a single `Arc<redb::Database>`. JSON serialization format is unchanged. All existing `UserRepository` and `RoleRepository` trait impls are preserved — callers are unaffected.

**Tech Stack:** redb 2.x, serde_json (unchanged), async-trait (unchanged), tokio (unchanged)

---

## File Map

| Action | Path |
|--------|------|
| Modify | `Cargo.toml` (workspace) |
| Modify | `carbon/Cargo.toml` |
| Create | `carbon/src/auth/redb_repository.rs` |
| Delete | `carbon/src/auth/sled_repository.rs` |
| Modify | `carbon/src/auth/error.rs` |
| Modify | `carbon/src/auth/mod.rs` |
| Create | `carbon/src/persistence/redb_store.rs` |
| Delete | `carbon/src/persistence/sled_store.rs` |
| Modify | `carbon/src/persistence/mod.rs` |

---

### Task 1: Swap dependency — sled → redb

**Files:**
- Modify: `Cargo.toml`
- Modify: `carbon/Cargo.toml`

- [ ] **Step 1: Replace sled with redb in workspace Cargo.toml**

In `Cargo.toml`, find and replace:
```toml
sled = "0.34"
```
With:
```toml
redb = "2"
```

- [ ] **Step 2: Replace sled with redb in carbon/Cargo.toml**

In `carbon/Cargo.toml`, find and replace:
```toml
sled.workspace = true
```
With:
```toml
redb.workspace = true
```

- [ ] **Step 3: Verify workspace resolves**

```bash
cargo fetch
```
Expected: no errors, redb 2.x downloaded.

---

### Task 2: Update error conversion in auth/error.rs

**Files:**
- Modify: `carbon/src/auth/error.rs`

- [ ] **Step 1: Replace the From<sled::Error> impl**

In `carbon/src/auth/error.rs`, replace:
```rust
impl From<sled::Error> for AuthError {
    fn from(err: sled::Error) -> Self {
        AuthError::StorageError(err.to_string())
    }
}
```
With:
```rust
impl From<redb::Error> for AuthError {
    fn from(err: redb::Error) -> Self {
        AuthError::StorageError(err.to_string())
    }
}

impl From<redb::TransactionError> for AuthError {
    fn from(err: redb::TransactionError) -> Self {
        AuthError::StorageError(err.to_string())
    }
}

impl From<redb::TableError> for AuthError {
    fn from(err: redb::TableError) -> Self {
        AuthError::StorageError(err.to_string())
    }
}

impl From<redb::StorageError> for AuthError {
    fn from(err: redb::StorageError) -> Self {
        AuthError::StorageError(err.to_string())
    }
}

impl From<redb::CommitError> for AuthError {
    fn from(err: redb::CommitError) -> Self {
        AuthError::StorageError(err.to_string())
    }
}
```

- [ ] **Step 2: Verify it compiles (ignoring sled_repository for now)**

```bash
cargo check -p carbon 2>&1 | grep "error\[" | head -20
```
Expected: errors only about `sled_repository` not yet updated — error.rs itself should be clean.

---

### Task 3: Create redb_repository.rs (auth)

**Files:**
- Create: `carbon/src/auth/redb_repository.rs`

- [ ] **Step 1: Write the failing tests first**

Create `carbon/src/auth/redb_repository.rs` with tests only:

```rust
use super::error::AuthError;
use super::models::{Role, User};
use super::repository::{RoleRepository, UserRepository};
use async_trait::async_trait;
use redb::{Database, TableDefinition};
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

#[derive(Clone)]
pub struct RedbRoleRepository {
    db: Arc<Database>,
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
```

- [ ] **Step 2: Run tests — verify they fail to compile**

```bash
cargo test -p carbon auth::redb_repository 2>&1 | head -30
```
Expected: compile error — `RedbUserRepository::new` not defined yet.

- [ ] **Step 3: Implement RedbUserRepository**

Add after the struct definitions (before `#[cfg(test)]`):

```rust
impl RedbUserRepository {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, AuthError> {
        let db = Database::create(path).map_err(|e| AuthError::StorageError(e.to_string()))?;
        let db = Arc::new(db);
        // Ensure tables exist
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
```

- [ ] **Step 4: Implement RedbRoleRepository**

Add after `RedbUserRepository` impl (before `#[cfg(test)]`):

```rust
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
```

- [ ] **Step 5: Run the tests — verify they pass**

```bash
cargo test -p carbon auth::redb_repository 2>&1
```
Expected: all 5 tests pass.

---

### Task 4: Update auth/mod.rs

**Files:**
- Modify: `carbon/src/auth/mod.rs`

- [ ] **Step 1: Swap module declaration and re-exports**

In `carbon/src/auth/mod.rs`, replace:
```rust
pub mod sled_repository;
```
With:
```rust
pub mod redb_repository;
```

And replace:
```rust
pub use sled_repository::{SledRoleRepository, SledUserRepository};
```
With:
```rust
pub use redb_repository::{RedbRoleRepository, RedbUserRepository};
```

- [ ] **Step 2: Check the crate compiles**

```bash
cargo check -p carbon 2>&1 | grep "error\["
```
Expected: errors only about `sled_repository.rs` still existing (unused) or callers using old names — we'll clean those up next.

---

### Task 5: Create redb_store.rs (persistence)

**Files:**
- Create: `carbon/src/persistence/redb_store.rs`

- [ ] **Step 1: Write failing tests first**

Create `carbon/src/persistence/redb_store.rs`:

```rust
use crate::domain::CacheConfig;
use redb::{Database, TableDefinition};
use shared::{Error, Result};
use std::path::Path;

const CONFIGS: TableDefinition<&str, &[u8]> = TableDefinition::new("configs");

pub struct RedbPersistence {
    db: Database,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::EvictionAlgorithm;
    use std::collections::HashMap;

    #[test]
    fn test_save_load_delete() {
        let dir = tempfile::tempdir().unwrap();
        let store = RedbPersistence::new(dir.path().join("test.redb")).unwrap();

        let config = CacheConfig::new(
            "my-cache",
            Some(1024 * 1024),
            None,
            Some(4),
            EvictionAlgorithm::TinyLfu,
            None,
            None,
            Some("desc".to_string()),
            Some(HashMap::from([("env".to_string(), "test".to_string())])),
        );

        store.save_config(&config).unwrap();

        let all = store.load_all().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].name, "my-cache");

        let fetched = store.get_config("my-cache").unwrap();
        assert!(fetched.is_some());

        let deleted = store.delete_config("my-cache").unwrap();
        assert!(deleted);

        let after = store.load_all().unwrap();
        assert_eq!(after.len(), 0);
    }

    #[test]
    fn test_delete_nonexistent_returns_false() {
        let dir = tempfile::tempdir().unwrap();
        let store = RedbPersistence::new(dir.path().join("test.redb")).unwrap();
        let deleted = store.delete_config("nope").unwrap();
        assert!(!deleted);
    }
}
```

- [ ] **Step 2: Run tests — verify they fail to compile**

```bash
cargo test -p carbon persistence::redb_store 2>&1 | head -20
```
Expected: compile error — `RedbPersistence::new` not defined.

- [ ] **Step 3: Implement RedbPersistence**

Add before `#[cfg(test)]` in `redb_store.rs`:

```rust
impl RedbPersistence {
    pub fn new(path: impl AsRef<Path>) -> Result<Self> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| Error::Internal(format!("Failed to create directory: {}", e)))?;
        }
        let db = Database::create(path)
            .map_err(|e| Error::Internal(format!("Failed to open redb database: {}", e)))?;
        let write_txn = db
            .begin_write()
            .map_err(|e| Error::Internal(format!("Failed to begin transaction: {}", e)))?;
        {
            write_txn
                .open_table(CONFIGS)
                .map_err(|e| Error::Internal(format!("Failed to open table: {}", e)))?;
        }
        write_txn
            .commit()
            .map_err(|e| Error::Internal(format!("Failed to commit: {}", e)))?;
        Ok(Self { db })
    }

    pub fn save_config(&self, config: &CacheConfig) -> Result<()> {
        let value = serde_json::to_vec(config)
            .map_err(|e| Error::Internal(format!("Failed to serialize config: {}", e)))?;
        let write_txn = self
            .db
            .begin_write()
            .map_err(|e| Error::Internal(format!("Failed to begin transaction: {}", e)))?;
        {
            let mut table = write_txn
                .open_table(CONFIGS)
                .map_err(|e| Error::Internal(format!("Failed to open table: {}", e)))?;
            table
                .insert(config.name.as_str(), value.as_slice())
                .map_err(|e| Error::Internal(format!("Failed to insert config: {}", e)))?;
        }
        write_txn
            .commit()
            .map_err(|e| Error::Internal(format!("Failed to commit: {}", e)))?;
        Ok(())
    }

    pub fn load_all(&self) -> Result<Vec<CacheConfig>> {
        let read_txn = self
            .db
            .begin_read()
            .map_err(|e| Error::Internal(format!("Failed to begin read: {}", e)))?;
        let table = read_txn
            .open_table(CONFIGS)
            .map_err(|e| Error::Internal(format!("Failed to open table: {}", e)))?;
        let mut configs = Vec::new();
        for entry in table
            .iter()
            .map_err(|e| Error::Internal(format!("Failed to iterate: {}", e)))?
        {
            let (_, v) = entry.map_err(|e| Error::Internal(format!("Failed to read entry: {}", e)))?;
            let config: CacheConfig = serde_json::from_slice(v.value())
                .map_err(|e| Error::Internal(format!("Failed to deserialize config: {}", e)))?;
            configs.push(config);
        }
        Ok(configs)
    }

    pub fn delete_config(&self, name: &str) -> Result<bool> {
        let write_txn = self
            .db
            .begin_write()
            .map_err(|e| Error::Internal(format!("Failed to begin transaction: {}", e)))?;
        let removed = {
            let mut table = write_txn
                .open_table(CONFIGS)
                .map_err(|e| Error::Internal(format!("Failed to open table: {}", e)))?;
            table
                .remove(name)
                .map_err(|e| Error::Internal(format!("Failed to delete config: {}", e)))?
                .is_some()
        };
        write_txn
            .commit()
            .map_err(|e| Error::Internal(format!("Failed to commit: {}", e)))?;
        Ok(removed)
    }

    pub fn get_config(&self, name: &str) -> Result<Option<CacheConfig>> {
        let read_txn = self
            .db
            .begin_read()
            .map_err(|e| Error::Internal(format!("Failed to begin read: {}", e)))?;
        let table = read_txn
            .open_table(CONFIGS)
            .map_err(|e| Error::Internal(format!("Failed to open table: {}", e)))?;
        match table
            .get(name)
            .map_err(|e| Error::Internal(format!("Failed to get config: {}", e)))?
        {
            Some(v) => {
                let config: CacheConfig = serde_json::from_slice(v.value())
                    .map_err(|e| Error::Internal(format!("Failed to deserialize config: {}", e)))?;
                Ok(Some(config))
            }
            None => Ok(None),
        }
    }
}
```

- [ ] **Step 4: Run tests — verify they pass**

```bash
cargo test -p carbon persistence::redb_store 2>&1
```
Expected: 2 tests pass.

---

### Task 6: Update persistence/mod.rs

**Files:**
- Modify: `carbon/src/persistence/mod.rs`

- [ ] **Step 1: Swap module and re-export**

Replace the entire contents of `carbon/src/persistence/mod.rs` with:

```rust
mod redb_store;

pub use redb_store::RedbPersistence;
```

- [ ] **Step 2: Check callers**

```bash
cargo check -p carbon 2>&1 | grep "error\["
```
Expected: errors only from callers still referencing `SledPersistence` — identify and note them for the next step.

---

### Task 7: Update all callers of renamed types

**Files:**
- Modify: any files referencing `SledPersistence`, `SledUserRepository`, `SledRoleRepository`

- [ ] **Step 1: Find all callsites**

```bash
grep -rn "SledPersistence\|SledUserRepository\|SledRoleRepository\|sled_repository\|sled_store" \
  /Users/chirdeeptomar/opensource/carbon --include="*.rs" \
  | grep -v "^Binary\|target/"
```

- [ ] **Step 2: Update each callsite**

For every file listed, replace:
- `SledPersistence` → `RedbPersistence`
- `SledUserRepository` → `RedbUserRepository`
- `SledRoleRepository` → `RedbRoleRepository`
- Any import paths `auth::sled_repository` → `auth::redb_repository`
- Any import paths `persistence::sled_store` → `persistence::redb_store`

- [ ] **Step 3: Full compile check**

```bash
cargo check --workspace 2>&1 | grep "error\["
```
Expected: no errors.

---

### Task 8: Delete old sled files and run full test suite

**Files:**
- Delete: `carbon/src/auth/sled_repository.rs`
- Delete: `carbon/src/persistence/sled_store.rs`

- [ ] **Step 1: Delete the old files**

```bash
rm /Users/chirdeeptomar/opensource/carbon/carbon/src/auth/sled_repository.rs
rm /Users/chirdeeptomar/opensource/carbon/carbon/src/persistence/sled_store.rs
```

- [ ] **Step 2: Run full test suite**

```bash
cargo test --workspace 2>&1
```
Expected: all tests pass, no compile errors.

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "feat: replace sled with redb, rename Sled* types to Redb*"
```
