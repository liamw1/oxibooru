use crate::model::user::{User, UserToken};
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use chrono::Utc;
use thiserror::Error;

#[derive(Debug, Error)]
#[error(transparent)]
pub enum AuthenticationError {
    EnvVar(#[from] std::env::VarError),
    Hash(#[from] argon2::password_hash::Error),
}

pub fn hash_password(password: &str, salt: &str) -> Result<String, AuthenticationError> {
    // TODO: Handle hash rotations
    let salt_and_pepper = std::env::var("PEPPER").map(|pepper| pepper + salt)?;
    let salt_string = SaltString::encode_b64(salt_and_pepper.as_bytes())?;
    let password_hash = argon_context().hash_password(password.as_bytes(), &salt_string)?;

    Ok(password_hash.to_string())
}

pub fn is_valid_password(user: &User, password: &str) -> bool {
    PasswordHash::new(&user.password_hash)
        .and_then(|parsed_hash| argon_context().verify_password(password.as_bytes(), &parsed_hash))
        .is_ok()
}

pub fn is_valid_token(user_token: &UserToken) -> bool {
    let expired = user_token.expiration_time.map_or(false, |time| time < Utc::now());
    user_token.enabled && !expired
}

fn argon_context() -> argon2::Argon2<'static> {
    Argon2::default()
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::test::*;
    use chrono::{DateTime, Days, Utc};
    use diesel::prelude::*;

    #[test]
    fn test_hash_password() {
        let user = establish_connection_or_panic().test_transaction(|conn| create_test_user(conn, TEST_USERNAME));
        assert!(is_valid_password(&user, TEST_PASSWORD))
    }

    #[test]
    fn test_is_valid_token() {
        let mut conn = establish_connection_or_panic();
        let future_time = Utc::now().checked_add_days(Days::new(1)).unwrap();

        let permanent_user_token = create_token(&mut conn, true, None);
        let temporary_user_token = create_token(&mut conn, true, Some(future_time));
        let expired_user_token = create_token(&mut conn, true, Some(Utc::now()));
        let disabled_user_token = create_token(&mut conn, false, None);

        assert!(is_valid_token(&permanent_user_token));
        assert!(is_valid_token(&temporary_user_token));
        assert!(!is_valid_token(&expired_user_token));
        assert!(!is_valid_token(&disabled_user_token));
    }

    fn create_token(conn: &mut PgConnection, enabled: bool, expiration_time: Option<DateTime<Utc>>) -> UserToken {
        conn.test_transaction(|conn| {
            create_test_user(conn, TEST_USERNAME)
                .and_then(|user| create_test_user_token(conn, &user, enabled, expiration_time))
        })
    }
}
