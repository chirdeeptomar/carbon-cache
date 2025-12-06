use super::models::User;
use chrono::DateTime;
use std::time::{SystemTime, UNIX_EPOCH};

/// Session token type - a secure random string
pub type SessionToken = String;

/// Get current timestamp in milliseconds since Unix epoch
pub fn current_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

/// Format a timestamp (ms since epoch) as ISO 8601 UTC string
pub fn format_utc_time(timestamp_ms: u64) -> String {
    let datetime = DateTime::from_timestamp_millis(timestamp_ms as i64)
        .unwrap_or_else(|| DateTime::from_timestamp(0, 0).unwrap());
    datetime.format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

/// Session data stored in cache with tracking metadata
#[derive(Debug, Clone)]
pub struct Session {
    pub token: SessionToken,
    pub user: User,
    pub created_at: u64,           // UTC timestamp in milliseconds
    pub created_at_utc: String,    // Human-readable UTC time (ISO 8601)
    pub expires_at: u64,            // UTC timestamp in milliseconds
    pub last_accessed: u64,         // UTC timestamp in milliseconds
    pub last_accessed_utc: String,  // Human-readable UTC time (ISO 8601)
    pub client_ip: Option<String>,  // IP address of the client
}

impl Session {
    /// Create a new session with the given token, user, TTL, and optional client IP
    pub fn new(token: SessionToken, user: User, ttl_ms: u64, client_ip: Option<String>) -> Self {
        let now = current_timestamp_ms();
        let now_utc = format_utc_time(now);

        Self {
            token,
            user,
            created_at: now,
            created_at_utc: now_utc.clone(),
            expires_at: now + ttl_ms,
            last_accessed: now,
            last_accessed_utc: now_utc,
            client_ip,
        }
    }

    /// Check if this session has expired
    pub fn is_expired(&self) -> bool {
        let now = current_timestamp_ms();
        now >= self.expires_at
    }

    /// Update the last_accessed timestamp to current time
    pub fn update_last_accessed(&mut self) {
        let now = current_timestamp_ms();
        self.last_accessed = now;
        self.last_accessed_utc = format_utc_time(now);
    }

    /// Get remaining time to live in milliseconds
    pub fn remaining_ttl_ms(&self) -> u64 {
        let now = current_timestamp_ms();

        if now >= self.expires_at {
            0
        } else {
            self.expires_at - now
        }
    }
}

/// Generate a cryptographically secure random session token
pub fn generate_session_token() -> SessionToken {
    use rand::Rng;

    // Generate 32 random bytes and encode as hex (64 characters)
    let mut rng = rand::rng();
    let bytes: Vec<u8> = (0..32).map(|_| rng.random()).collect();

    // Convert to hex string
    bytes
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<String>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_session_token() {
        let token1 = generate_session_token();
        let token2 = generate_session_token();

        // Should be 64 characters (32 bytes as hex)
        assert_eq!(token1.len(), 64);
        assert_eq!(token2.len(), 64);

        // Should be different
        assert_ne!(token1, token2);

        // Should be valid hex
        assert!(token1.chars().all(|c| c.is_ascii_hexdigit()));
        assert!(token2.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_session_creation() {
        let token = "test_token_123".to_string();
        let user = User::new("testuser".to_string(), "hash".to_string(), vec![]);
        let ttl_ms = 3600000; // 1 hour

        let session = Session::new(token.clone(), user.clone(), ttl_ms, Some("127.0.0.1".to_string()));

        assert_eq!(session.token, token);
        assert_eq!(session.user.username, "testuser");
        assert_eq!(session.client_ip, Some("127.0.0.1".to_string()));
        assert!(!session.is_expired());
        assert!(session.remaining_ttl_ms() > 0);
    }

    #[test]
    fn test_session_expiration() {
        let token = "test_token_123".to_string();
        let user = User::new("testuser".to_string(), "hash".to_string(), vec![]);
        let ttl_ms = 0; // Already expired

        let session = Session::new(token, user, ttl_ms, None);

        assert!(session.is_expired());
        assert_eq!(session.remaining_ttl_ms(), 0);
    }

    #[test]
    fn test_session_remaining_ttl() {
        let token = "test_token_123".to_string();
        let user = User::new("testuser".to_string(), "hash".to_string(), vec![]);
        let ttl_ms = 5000; // 5 seconds

        let session = Session::new(token, user, ttl_ms, None);

        let remaining = session.remaining_ttl_ms();
        assert!(remaining > 4000 && remaining <= 5000);
    }
}