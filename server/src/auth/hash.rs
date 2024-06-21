use crate::auth::HashError;
use crate::config::CONFIG;
use crate::model::user::{User, UserToken};
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use argon2::{Algorithm, Params, Version};
use once_cell::sync::Lazy;
use time::OffsetDateTime;

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

pub fn is_valid_token(user_token: &UserToken) -> bool {
    let expired = user_token
        .expiration_time
        .map_or(false, |time| time < OffsetDateTime::now_utc());
    user_token.enabled && !expired
}

static PEPPER: Lazy<&'static str> = Lazy::new(|| {
    CONFIG
        .get("secret")
        .and_then(|parsed| parsed.as_str())
        .unwrap_or_else(|| panic!("No secret found in config.toml"))
});
static ARGON_CONTEXT: Lazy<Argon2> = Lazy::new(|| {
    Argon2::new_with_secret(PEPPER.as_bytes(), Algorithm::default(), Version::default(), Params::default())
        .unwrap_or_else(|err| panic!("{err}"))
});

#[cfg(test)]
mod test {
    use super::*;
    use crate::test::*;
    use time::Duration;

    #[test]
    fn hash_password() {
        let user = test_transaction(|conn| create_test_user(conn, TEST_USERNAME));
        assert!(is_valid_password(&user, TEST_PASSWORD))
    }

    #[test]
    fn validate_token() {
        let future_time = OffsetDateTime::now_utc().checked_add(Duration::DAY).unwrap();

        let permanent_user_token = create_token(true, None);
        let temporary_user_token = create_token(true, Some(future_time));
        let expired_user_token = create_token(true, Some(OffsetDateTime::now_utc()));
        let disabled_user_token = create_token(false, None);

        assert!(is_valid_token(&permanent_user_token));
        assert!(is_valid_token(&temporary_user_token));
        assert!(!is_valid_token(&expired_user_token));
        assert!(!is_valid_token(&disabled_user_token));
    }

    fn create_token(enabled: bool, expiration_time: Option<OffsetDateTime>) -> UserToken {
        test_transaction(|conn| {
            create_test_user(conn, TEST_USERNAME)
                .and_then(|user| create_test_user_token(conn, &user, enabled, expiration_time))
        })
    }
}
