use thiserror::Error;

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("Invalid credentials")]
    InvalidCredentials,

    #[error("Permission denied")]
    PermissionDenied,

    #[error("User not found")]
    UserNotFound,

    #[error("Role not found")]
    RoleNotFound,

    #[error("User already exists")]
    UserAlreadyExists,

    #[error("Role already exists")]
    RoleAlreadyExists,

    #[error("Password does not meet strength requirements")]
    WeakPassword,

    #[error("Cannot delete system role")]
    CannotDeleteSystemRole,

    #[error("Cannot delete user's own account")]
    CannotDeleteSelf,

    #[error("Invalid role assignment")]
    InvalidRoleAssignment,

    #[error("Storage error: {0}")]
    StorageError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Password hashing error: {0}")]
    PasswordHashError(String),
}

impl From<sled::Error> for AuthError {
    fn from(err: sled::Error) -> Self {
        AuthError::StorageError(err.to_string())
    }
}

impl From<serde_json::Error> for AuthError {
    fn from(err: serde_json::Error) -> Self {
        AuthError::SerializationError(err.to_string())
    }
}
