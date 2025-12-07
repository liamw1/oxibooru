use crate::config::Config;
use argon2::password_hash::{Error, PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::{Algorithm, Params, Version};
use argon2::{Argon2, Error as ArgonError};

/// Takes a plaintext `password` and hashes it using a cryptographically secure,
/// memory-hard hash: Argon2id. A randomly generated `salt` is mixed in with the
/// hash to protect against rainbow table attacks.
pub fn hash_password(config: &Config, password: &str, salt: &SaltString) -> Result<String, Error> {
    let argon_context = create_argon_context(config)?;
    let password_hash = argon_context.hash_password(password.as_bytes(), salt)?;
    Ok(password_hash.to_string())
}

/// Returns [`Ok`] if the given `user` and `password` match.
pub fn is_valid_password(config: &Config, password_hash: &str, password: &str) -> Result<(), Error> {
    let argon_context = create_argon_context(config)?;
    PasswordHash::new(password_hash)
        .and_then(|parsed_hash| argon_context.verify_password(password.as_bytes(), &parsed_hash))
}

fn create_argon_context(config: &Config) -> Result<Argon2<'_>, ArgonError> {
    Argon2::new_with_secret(
        config.password_secret.as_bytes(),
        Algorithm::default(),
        Version::default(),
        Params::default(),
    )
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::config;
    use crate::test::*;

    #[test]
    fn hash_password() {
        let test_config = config::test_config(None);
        assert!(is_valid_password(&test_config, TEST_HASH, TEST_PASSWORD).is_ok());
        assert!(is_valid_password(&test_config, TEST_HASH, "wrong_password").is_err());
    }
}
