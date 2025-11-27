use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use argon2::password_hash::rand_core::OsRng;

use super::error::AuthError;

/// Hash a password using Argon2 with secure defaults
pub fn hash_password(password: &str) -> Result<String, AuthError> {
    validate_password_strength(password)?;

    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();

    argon2
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|e| AuthError::PasswordHashError(e.to_string()))
}

/// Verify a password against a hash using constant-time comparison
pub fn verify_password(password: &str, hash: &str) -> Result<bool, AuthError> {
    let parsed_hash =
        PasswordHash::new(hash).map_err(|e| AuthError::PasswordHashError(e.to_string()))?;

    let argon2 = Argon2::default();

    match argon2.verify_password(password.as_bytes(), &parsed_hash) {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}

/// Validate password strength requirements
pub fn validate_password_strength(password: &str) -> Result<(), AuthError> {
    if password.len() < 8 {
        return Err(AuthError::WeakPassword);
    }

    // Check for at least one letter and one number
    let has_letter = password.chars().any(|c| c.is_alphabetic());
    let has_number = password.chars().any(|c| c.is_numeric());

    if !has_letter || !has_number {
        return Err(AuthError::WeakPassword);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_and_verify_password() {
        let password = "test_password123";
        let hash = hash_password(password).unwrap();

        assert!(verify_password(password, &hash).unwrap());
        assert!(!verify_password("wrong_password", &hash).unwrap());
    }

    #[test]
    fn test_password_validation() {
        assert!(validate_password_strength("test1234").is_ok());
        assert!(validate_password_strength("TestPass123").is_ok());

        // Too short
        assert!(validate_password_strength("test1").is_err());

        // No number
        assert!(validate_password_strength("testpassword").is_err());

        // No letter
        assert!(validate_password_strength("12345678").is_err());
    }

    #[test]
    fn test_different_hashes_for_same_password() {
        let password = "test_password123";
        let hash1 = hash_password(password).unwrap();
        let hash2 = hash_password(password).unwrap();

        // Different hashes due to different salts
        assert_ne!(hash1, hash2);

        // Both should verify successfully
        assert!(verify_password(password, &hash1).unwrap());
        assert!(verify_password(password, &hash2).unwrap());
    }
}
