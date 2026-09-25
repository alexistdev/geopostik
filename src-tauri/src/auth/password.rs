use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng};
use argon2::Argon2;

use crate::error::{AppError, AppResult};

/// Hash password atau PIN dengan argon2id.
pub fn hash(secret: &str) -> AppResult<String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(secret.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| AppError::Internal(format!("gagal hash password: {e}")))
}

pub fn verify(secret: &str, hash: &str) -> bool {
    PasswordHash::new(hash)
        .map(|parsed| Argon2::default().verify_password(secret.as_bytes(), &parsed).is_ok())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_and_verify() {
        let h = hash("rahasia123").unwrap();
        assert!(verify("rahasia123", &h));
        assert!(!verify("salah", &h));
        assert!(!verify("rahasia123", "bukan-hash"));
    }
}
