use std::sync::Arc;

use async_trait::async_trait;
use carbon::auth::{
    error::AuthError,
    repository::{RoleRepository, UserRepository},
    Role, User,
};
use tokio::sync::RwLock;

use crate::state_machine::CacheStateMachine;

/// Read-only [`UserRepository`] backed by the Raft state machine.
///
/// Write methods (`create`, `update`, `delete`) intentionally return an error
/// directing callers to use [`RaftCacheNode::write`] instead, which routes
/// mutations through Raft consensus so they are replicated to all nodes.
pub struct RaftUserRepository {
    pub sm: Arc<RwLock<CacheStateMachine>>,
}

/// Read-only [`RoleRepository`] backed by the Raft state machine.
///
/// Same constraint as [`RaftUserRepository`]: write methods return an error;
/// use [`RaftCacheNode::write`] for mutations.
pub struct RaftRoleRepository {
    pub sm: Arc<RwLock<CacheStateMachine>>,
}

#[async_trait]
impl UserRepository for RaftUserRepository {
    async fn create(&self, _user: User) -> Result<User, AuthError> {
        Err(AuthError::StorageError(
            "use RaftCacheNode::write(CreateUser) in cluster mode".into(),
        ))
    }

    async fn find_by_username(&self, username: &str) -> Result<Option<User>, AuthError> {
        Ok(self.sm.read().await.get_user_by_username(username).cloned())
    }

    async fn find_by_id(&self, id: &str) -> Result<Option<User>, AuthError> {
        Ok(self.sm.read().await.get_user_by_id(id).cloned())
    }

    async fn list_all(&self) -> Result<Vec<User>, AuthError> {
        Ok(self.sm.read().await.list_users())
    }

    async fn update(&self, _user: User) -> Result<User, AuthError> {
        Err(AuthError::StorageError(
            "use RaftCacheNode::write(UpdateUser) in cluster mode".into(),
        ))
    }

    async fn delete(&self, _id: &str) -> Result<(), AuthError> {
        Err(AuthError::StorageError(
            "use RaftCacheNode::write(DeleteUser) in cluster mode".into(),
        ))
    }

    async fn username_exists(&self, username: &str) -> Result<bool, AuthError> {
        Ok(self.sm.read().await.username_exists(username))
    }
}

#[async_trait]
impl RoleRepository for RaftRoleRepository {
    async fn create(&self, _role: Role) -> Result<Role, AuthError> {
        Err(AuthError::StorageError(
            "use RaftCacheNode::write(CreateRole) in cluster mode".into(),
        ))
    }

    async fn find_by_name(&self, name: &str) -> Result<Option<Role>, AuthError> {
        Ok(self.sm.read().await.get_role_by_name(name).cloned())
    }

    async fn find_by_id(&self, id: &str) -> Result<Option<Role>, AuthError> {
        Ok(self.sm.read().await.get_role_by_id(id).cloned())
    }

    async fn find_by_ids(&self, ids: &[String]) -> Result<Vec<Role>, AuthError> {
        Ok(self.sm.read().await.get_roles_by_ids(ids))
    }

    async fn list_all(&self) -> Result<Vec<Role>, AuthError> {
        Ok(self.sm.read().await.list_roles())
    }

    async fn update(&self, _role: Role) -> Result<Role, AuthError> {
        Err(AuthError::StorageError(
            "use RaftCacheNode::write(UpdateRole) in cluster mode".into(),
        ))
    }

    async fn delete(&self, _id: &str) -> Result<(), AuthError> {
        Err(AuthError::StorageError(
            "use RaftCacheNode::write(DeleteRole) in cluster mode".into(),
        ))
    }

    async fn name_exists(&self, name: &str) -> Result<bool, AuthError> {
        Ok(self.sm.read().await.role_name_exists(name))
    }
}
