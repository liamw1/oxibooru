use crate::auth::HashError;
use crate::config;
use argon2::Argon2;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::{Algorithm, Params, Version};
use std::sync::LazyLock;

/// Takes a plaintext `password` and hashes it using a cryptographically secure,
/// memory-hard hash: Argon2id. A randomly generated `salt` is mixed in with the
/// hash to protect against rainbow table attacks.
pub fn hash_password(password: &str, salt: &SaltString) -> Result<String, HashError> {
    let password_hash = ARGON_CONTEXT.hash_password(password.as_bytes(), salt)?;
    Ok(password_hash.to_string())
}

/// Returns if the given `user` and `password` match.
pub fn is_valid_password(password_hash: &str, password: &str) -> bool {
    PasswordHash::new(password_hash)
        .and_then(|parsed_hash| ARGON_CONTEXT.verify_password(password.as_bytes(), &parsed_hash))
        .is_ok()
}

static ARGON_CONTEXT: LazyLock<Argon2> = LazyLock::new(|| {
    Argon2::new_with_secret(
        config::get().password_secret.as_bytes(),
        Algorithm::default(),
        Version::default(),
        Params::default(),
    )
    .unwrap()
});

#[cfg(test)]
mod test {
    use super::*;
    use crate::test::*;

    #[test]
    fn hash_password() {
        assert!(is_valid_password(TEST_HASH, TEST_PASSWORD));
        assert!(!is_valid_password(TEST_HASH, "wrong_password"));
    }
}
