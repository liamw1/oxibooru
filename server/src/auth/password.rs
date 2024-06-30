use crate::auth::HashError;
use crate::config;
use crate::model::user::User;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use argon2::{Algorithm, Params, Version};
use once_cell::sync::Lazy;

pub fn hash_password(password: &str, salt: &str) -> Result<String, HashError> {
    // TODO: Handle hash rotations
    let salt_string = SaltString::encode_b64(salt.as_bytes())?;
    let password_hash = ARGON_CONTEXT.hash_password(password.as_bytes(), &salt_string)?;

    Ok(password_hash.to_string())
}

pub fn is_valid_password(user: &User, password: &str) -> bool {
    PasswordHash::new(&user.password_hash)
        .and_then(|parsed_hash| ARGON_CONTEXT.verify_password(password.as_bytes(), &parsed_hash))
        .is_ok()
}

static SECRET: Lazy<&'static str> = Lazy::new(|| config::read_required_string("password_secret"));
static ARGON_CONTEXT: Lazy<Argon2> = Lazy::new(|| {
    Argon2::new_with_secret(SECRET.as_bytes(), Algorithm::default(), Version::default(), Params::default()).unwrap()
});

#[cfg(test)]
mod test {
    use super::*;
    use crate::test::*;

    #[test]
    fn hash_password() {
        let user = test_transaction(|conn| create_test_user(conn, TEST_USERNAME));
        assert!(is_valid_password(&user, TEST_PASSWORD))
    }
}
