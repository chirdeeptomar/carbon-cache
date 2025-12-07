use super::models::User;
use super::session::{generate_session_token, Session, SessionToken};
use super::session_store::SessionRepository;
use async_trait::async_trait;
use moka::future::Cache;
use parking_lot::RwLock;
use shared::Result;
use std::sync::Arc;
use std::time::Duration;

/// Username type alias
pub type Username = String;

/// Moka-based in-memory session repository with dual-index support
pub struct MokaSessionRepository {
    // Primary index: token -> session
    sessions: Cache<SessionToken, Session>,
    // Secondary index: username -> list of session tokens
    user_sessions: Cache<Username, Arc<RwLock<Vec<SessionToken>>>>,
}

impl MokaSessionRepository {
    /// Create a new Moka session repository with specified capacity and default TTL
    pub fn new(max_sessions: Option<u64>, default_ttl: Option<Duration>) -> Self {
        let mut sessions_builder = Cache::builder();
        let mut user_sessions_builder = Cache::builder();

        if let Some(capacity) = max_sessions {
            sessions_builder = sessions_builder.max_capacity(capacity);
            // User sessions cache can be smaller (assume 10 sessions per user on average)
            user_sessions_builder = user_sessions_builder.max_capacity(capacity / 10);
        }

        if let Some(ttl) = default_ttl {
            sessions_builder = sessions_builder.time_to_live(ttl);
            user_sessions_builder = user_sessions_builder.time_to_live(ttl);
        }

        Self {
            sessions: sessions_builder.build(),
            user_sessions: user_sessions_builder.build(),
        }
    }

    /// Create with default settings (unbounded, 1 hour TTL)
    pub fn with_defaults() -> Self {
        Self::new(None, Some(Duration::from_secs(3600)))
    }
}

#[async_trait]
impl SessionRepository for MokaSessionRepository {
    async fn create_session(&self, user: User, ttl_ms: u64, client_ip: Option<String>) -> Result<Session> {
        let token = generate_session_token();
        let session = Session::new(token.clone(), user.clone(), ttl_ms, client_ip);

        // Store session in primary index
        self.sessions.insert(token.clone(), session.clone()).await;

        // Add token to user's session list in secondary index
        let username = user.username.clone();
        let tokens_lock = self.user_sessions
            .get(&username)
            .await
            .unwrap_or_else(|| Arc::new(RwLock::new(Vec::new())));

        {
            let mut tokens = tokens_lock.write();
            tokens.push(token.clone());
        }

        self.user_sessions.insert(username, tokens_lock).await;

        Ok(session)
    }

    async fn get_session(&self, token: &SessionToken) -> Result<User> {
        let session = self.sessions
            .get(token)
            .await
            .ok_or(shared::Error::NotFound)?;

        // Check if expired
        if session.is_expired() {
            self.sessions.invalidate(token).await;
            return Err(shared::Error::NotFound);
        }

        // Update last_accessed timestamp
        let mut updated = session.clone();
        updated.update_last_accessed();
        self.sessions.insert(token.clone(), updated).await;

        Ok(session.user)
    }

    async fn delete_session(&self, token: &SessionToken) -> Result<bool> {
        let session = self.sessions.remove(token).await;

        if let Some(data) = &session {
            // Remove from username index
            if let Some(tokens_lock) = self.user_sessions.get(&data.user.username).await {
                let mut tokens = tokens_lock.write();
                tokens.retain(|t| t != token);
            }
        }

        Ok(session.is_some())
    }

    async fn session_exists(&self, token: &SessionToken) -> Result<bool> {
        if let Some(session) = self.sessions.get(token).await {
            Ok(!session.is_expired())
        } else {
            Ok(false)
        }
    }

    async fn get_or_create_user_session(&self, user: User, ttl_ms: u64, client_ip: Option<String>) -> Result<Session> {
        let username = &user.username;

        // Try to get existing sessions for this user
        if let Some(tokens_lock) = self.user_sessions.get(username).await {
            // Clone tokens to release lock before awaiting
            let token_list: Vec<SessionToken> = {
                let tokens = tokens_lock.read();
                tokens.clone()
            };

            let mut most_recent: Option<Session> = None;
            let mut valid_tokens = Vec::new();

            // Find most recently accessed valid session
            for token in token_list.iter() {
                if let Some(session) = self.sessions.get(token).await {
                    if !session.is_expired() {
                        valid_tokens.push(token.clone());

                        if most_recent.is_none() ||
                           session.last_accessed > most_recent.as_ref().unwrap().last_accessed {
                            most_recent = Some(session);
                        }
                    }
                }
            }

            // Cleanup expired tokens (lazy cleanup)
            let needs_cleanup = valid_tokens.len() != token_list.len();
            if needs_cleanup {
                let mut tokens_write = tokens_lock.write();
                *tokens_write = valid_tokens;
            }

            // Return most recent session if found
            if let Some(mut session) = most_recent {
                session.update_last_accessed();
                self.sessions.insert(session.token.clone(), session.clone()).await;
                return Ok(session);
            }
        }

        // No valid session found - create new one
        self.create_session(user, ttl_ms, client_ip).await
    }

    async fn get_user_sessions(&self, username: &str) -> Result<Vec<Session>> {
        let mut sessions = Vec::new();

        if let Some(tokens_lock) = self.user_sessions.get(username).await {
            // Clone tokens to release lock before awaiting
            let token_list: Vec<SessionToken> = {
                let tokens = tokens_lock.read();
                tokens.clone()
            };

            for token in token_list.iter() {
                if let Some(session) = self.sessions.get(token).await {
                    if !session.is_expired() {
                        sessions.push(session);
                    }
                }
            }
        }

        Ok(sessions)
    }

    async fn delete_user_sessions(&self, username: &str) -> Result<usize> {
        let mut count = 0;

        if let Some(tokens_lock) = self.user_sessions.get(username).await {
            // Clone tokens to release lock before awaiting
            let token_list: Vec<SessionToken> = {
                let tokens = tokens_lock.read();
                tokens.clone()
            };

            for token in token_list.iter() {
                if self.sessions.remove(token).await.is_some() {
                    count += 1;
                }
            }

            self.user_sessions.invalidate(username).await;
        }

        Ok(count)
    }

    async fn get_existing_user_session(&self, username: &str) -> Result<Option<Session>> {
        if let Some(tokens_lock) = self.user_sessions.get(username).await {
            // Clone tokens to release lock before awaiting
            let token_list: Vec<SessionToken> = {
                let tokens = tokens_lock.read();
                tokens.clone()
            };

            // Find most recently accessed valid session
            let mut most_recent: Option<Session> = None;

            for token in token_list.iter() {
                if let Some(session) = self.sessions.get(token).await {
                    if !session.is_expired() {
                        if most_recent.is_none() ||
                           session.last_accessed > most_recent.as_ref().unwrap().last_accessed {
                            most_recent = Some(session);
                        }
                    }
                }
            }

            Ok(most_recent)
        } else {
            Ok(None)
        }
    }

    async fn update_session(&self, session: &Session) -> Result<()> {
        self.sessions.insert(session.token.clone(), session.clone()).await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_create_and_validate_session() {
        let repo = MokaSessionRepository::with_defaults();
        let user = User::new("testuser".to_string(), "hash".to_string(), vec![]);

        // Create session
        let session = repo.create_session(user.clone(), 3600000, Some("192.168.1.1".to_string())).await.unwrap();
        assert!(!session.token.is_empty());

        // Validate session
        let retrieved_user = repo.get_session(&session.token).await.unwrap();
        assert_eq!(retrieved_user.username, "testuser");
    }

    #[tokio::test]
    async fn test_delete_session() {
        let repo = MokaSessionRepository::with_defaults();
        let user = User::new("testuser".to_string(), "hash".to_string(), vec![]);

        // Create session
        let session = repo.create_session(user, 3600000, None).await.unwrap();

        // Delete session
        let deleted = repo.delete_session(&session.token).await.unwrap();
        assert!(deleted);

        // Should not exist anymore
        let result = repo.get_session(&session.token).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_session_expiration() {
        let repo = MokaSessionRepository::with_defaults();
        let user = User::new("testuser".to_string(), "hash".to_string(), vec![]);

        // Create session with 0ms TTL (immediately expired)
        let session = repo.create_session(user, 0, None).await.unwrap();

        // Should be expired
        assert!(!repo.session_exists(&session.token).await.unwrap());
    }

    #[tokio::test]
    async fn test_session_reuse_for_same_user() {
        let repo = MokaSessionRepository::with_defaults();
        let user = User::new("testuser".to_string(), "hash".to_string(), vec![]);

        // Create first session
        let session1 = repo.get_or_create_user_session(user.clone(), 3600000, Some("192.168.1.1".to_string())).await.unwrap();

        // Request session again - should return same token
        let session2 = repo.get_or_create_user_session(user.clone(), 3600000, Some("192.168.1.2".to_string())).await.unwrap();

        assert_eq!(session1.token, session2.token);
        assert_eq!(session1.created_at, session2.created_at);
    }

    #[tokio::test]
    async fn test_multiple_concurrent_sessions() {
        let repo = MokaSessionRepository::with_defaults();
        let user = User::new("testuser".to_string(), "hash".to_string(), vec![]);

        // Create 3 sessions explicitly
        let session1 = repo.create_session(user.clone(), 3600000, Some("192.168.1.1".to_string())).await.unwrap();
        let session2 = repo.create_session(user.clone(), 3600000, Some("192.168.1.2".to_string())).await.unwrap();
        let session3 = repo.create_session(user.clone(), 3600000, Some("192.168.1.3".to_string())).await.unwrap();

        // All should be different
        assert_ne!(session1.token, session2.token);
        assert_ne!(session2.token, session3.token);

        // Get user sessions - should return all 3
        let sessions = repo.get_user_sessions("testuser").await.unwrap();
        assert_eq!(sessions.len(), 3);
    }

    #[tokio::test]
    async fn test_most_recently_accessed_selection() {
        let repo = MokaSessionRepository::with_defaults();
        let user = User::new("testuser".to_string(), "hash".to_string(), vec![]);

        // Create 2 sessions
        let session1 = repo.create_session(user.clone(), 3600000, None).await.unwrap();
        let _session2 = repo.create_session(user.clone(), 3600000, None).await.unwrap();

        // Access session1 (updates last_accessed)
        repo.get_session(&session1.token).await.unwrap();

        // get_or_create should return session1 (most recently accessed)
        let selected = repo.get_or_create_user_session(user.clone(), 3600000, None).await.unwrap();
        assert_eq!(selected.token, session1.token);
    }

    #[tokio::test]
    async fn test_expired_session_cleanup() {
        let repo = MokaSessionRepository::with_defaults();
        let user = User::new("testuser".to_string(), "hash".to_string(), vec![]);

        // Create session with 0ms TTL (immediately expired)
        let session = repo.create_session(user.clone(), 0, None).await.unwrap();

        // get_or_create should create NEW session (old one expired)
        let new_session = repo.get_or_create_user_session(user.clone(), 3600000, None).await.unwrap();
        assert_ne!(session.token, new_session.token);

        // User should only have 1 valid session (expired one filtered out)
        let sessions = repo.get_user_sessions("testuser").await.unwrap();
        assert_eq!(sessions.len(), 1);
    }

    #[tokio::test]
    async fn test_delete_user_sessions() {
        let repo = MokaSessionRepository::with_defaults();
        let user = User::new("testuser".to_string(), "hash".to_string(), vec![]);

        // Create 3 sessions
        repo.create_session(user.clone(), 3600000, None).await.unwrap();
        repo.create_session(user.clone(), 3600000, None).await.unwrap();
        repo.create_session(user.clone(), 3600000, None).await.unwrap();

        // Delete all sessions
        let deleted = repo.delete_user_sessions("testuser").await.unwrap();
        assert_eq!(deleted, 3);

        // User should have no sessions
        let sessions = repo.get_user_sessions("testuser").await.unwrap();
        assert_eq!(sessions.len(), 0);
    }

    #[tokio::test]
    async fn test_different_users_isolated_sessions() {
        let repo = MokaSessionRepository::with_defaults();

        let user1 = User::new("user1".to_string(), "hash1".to_string(), vec![]);
        let user2 = User::new("user2".to_string(), "hash2".to_string(), vec![]);

        // Create sessions for both users
        let session1 = repo.get_or_create_user_session(user1.clone(), 3600000, None).await.unwrap();
        let session2 = repo.get_or_create_user_session(user2.clone(), 3600000, None).await.unwrap();

        // Sessions should be different
        assert_ne!(session1.token, session2.token);

        // Each user should have exactly 1 session
        let user1_sessions = repo.get_user_sessions("user1").await.unwrap();
        let user2_sessions = repo.get_user_sessions("user2").await.unwrap();

        assert_eq!(user1_sessions.len(), 1);
        assert_eq!(user2_sessions.len(), 1);
    }
}
