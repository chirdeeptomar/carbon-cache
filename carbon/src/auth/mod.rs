// Public API
pub mod auth_service;
pub mod defaults;
pub mod error;
pub mod models;
pub mod moka_session_repository;
pub mod password;
pub mod redb_repository;
pub mod repository;
pub mod role_service;
pub mod session;
pub mod session_store;
pub mod user_service;

// Re-export commonly used types
pub use auth_service::AuthService;
pub use error::AuthError;
pub use models::{Permission, Role, User};
pub use moka_session_repository::MokaSessionRepository;
pub use redb_repository::{RedbRoleRepository, RedbUserRepository};
pub use repository::{RoleRepository, UserRepository};
pub use role_service::RoleService;
pub use session::{
    Session, SessionToken, current_timestamp_ms, format_utc_time, generate_session_token,
};
pub use session_store::{SessionRepository, SessionStore};
pub use user_service::UserService;
