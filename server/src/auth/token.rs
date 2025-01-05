use crate::model::user::UserToken;
use crate::time::DateTime;

/// Returns if the given `user_token` is enabled and has not expired.
pub fn is_valid_token(user_token: &UserToken) -> bool {
    let expired = user_token
        .expiration_time
        .as_ref()
        .map_or(false, |&time| time < DateTime::now());
    user_token.enabled && !expired
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::test::*;
    use time::Duration;

    #[test]
    fn validate_token() {
        let future_time = DateTime::now().checked_add(Duration::DAY).unwrap().into();

        let permanent_user_token = create_token(true, None);
        let temporary_user_token = create_token(true, Some(future_time));
        let expired_user_token = create_token(true, Some(DateTime::now()));
        let disabled_user_token = create_token(false, None);

        assert!(is_valid_token(&permanent_user_token));
        assert!(is_valid_token(&temporary_user_token));
        assert!(!is_valid_token(&expired_user_token));
        assert!(!is_valid_token(&disabled_user_token));
    }

    fn create_token(enabled: bool, expiration_time: Option<DateTime>) -> UserToken {
        test_transaction(|conn| {
            create_test_user(conn, TEST_USERNAME)
                .and_then(|user| create_test_user_token(conn, &user, enabled, expiration_time))
        })
    }
}
