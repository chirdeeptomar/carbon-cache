// Public API
pub mod auth_service;
pub mod defaults;
pub mod error;
pub mod models;
pub mod password;
pub mod repository;
pub mod role_service;
pub mod sled_repository;
pub mod user_service;

// Re-export commonly used types
pub use auth_service::AuthService;
pub use error::AuthError;
pub use models::{Permission, Role, User};
pub use repository::{RoleRepository, UserRepository};
pub use role_service::RoleService;
pub use sled_repository::{SledRoleRepository, SledUserRepository};
pub use user_service::UserService;
