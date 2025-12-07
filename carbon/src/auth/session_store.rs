use super::models::User;
use super::session::{Session, SessionToken};
use async_trait::async_trait;
use shared::Result;
use std::sync::Arc;

/// Trait for session storage operations
#[async_trait]
pub trait SessionRepository: Send + Sync {
    /// Create a new session for the given user with specified TTL and optional client IP
    async fn create_session(&self, user: User, ttl_ms: u64, client_ip: Option<String>) -> Result<Session>;

    /// Get a session by token
    async fn get_session(&self, token: &SessionToken) -> Result<User>;

    /// Delete a session (logout)
    async fn delete_session(&self, token: &SessionToken) -> Result<bool>;

    /// Check if a session exists
    async fn session_exists(&self, token: &SessionToken) -> Result<bool>;

    /// Get or create a session for a user (for transparent session management)
    /// If user has existing valid sessions, returns the most recently accessed one
    /// Otherwise creates a new session
    async fn get_or_create_user_session(&self, user: User, ttl_ms: u64, client_ip: Option<String>) -> Result<Session>;

    /// Get all active sessions for a user
    async fn get_user_sessions(&self, username: &str) -> Result<Vec<Session>>;

    /// Delete all sessions for a user (for password change, logout all)
    async fn delete_user_sessions(&self, username: &str) -> Result<usize>;

    /// Get existing valid session for username without password verification
    /// Returns most recently accessed session if one exists, None otherwise
    /// This is used for fast-path authentication when session already exists
    async fn get_existing_user_session(&self, username: &str) -> Result<Option<Session>>;

    /// Update session's last_accessed timestamp
    async fn update_session(&self, session: &Session) -> Result<()>;
}

/// Session store service
pub struct SessionStore<S: SessionRepository> {
    repository: Arc<S>,
}

impl<S: SessionRepository> SessionStore<S> {
    pub fn new(repository: Arc<S>) -> Self {
        Self { repository }
    }

    /// Create a new session for a user with optional client IP
    pub async fn create_session(&self, user: User, ttl_ms: u64, client_ip: Option<String>) -> Result<Session> {
        self.repository.create_session(user, ttl_ms, client_ip).await
    }

    /// Validate a session token and return the associated user
    pub async fn validate_session(&self, token: &SessionToken) -> Result<User> {
        self.repository.get_session(token).await
    }

    /// Invalidate a session (logout)
    pub async fn invalidate_session(&self, token: &SessionToken) -> Result<bool> {
        self.repository.delete_session(token).await
    }

    /// Check if a session token is valid
    pub async fn is_valid_session(&self, token: &SessionToken) -> Result<bool> {
        self.repository.session_exists(token).await
    }

    /// Get or create a session for a user (transparent session management)
    /// If user has existing valid sessions, returns the most recently accessed one
    /// Otherwise creates a new session
    pub async fn get_or_create_user_session(&self, user: User, ttl_ms: u64, client_ip: Option<String>) -> Result<Session> {
        self.repository.get_or_create_user_session(user, ttl_ms, client_ip).await
    }

    /// Get all active sessions for a user
    pub async fn get_user_sessions(&self, username: &str) -> Result<Vec<Session>> {
        self.repository.get_user_sessions(username).await
    }

    /// Invalidate all sessions for a user (logout all devices)
    pub async fn invalidate_user_sessions(&self, username: &str) -> Result<usize> {
        self.repository.delete_user_sessions(username).await
    }

    /// Get existing valid session for username (fast path - no password verification)
    /// Returns most recently accessed session if one exists
    pub async fn get_existing_user_session(&self, username: &str) -> Result<Option<Session>> {
        self.repository.get_existing_user_session(username).await
    }

    /// Update session's last_accessed timestamp
    pub async fn update_session(&self, session: &Session) -> Result<()> {
        self.repository.update_session(session).await
    }
}