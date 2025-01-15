use crate::content::hash;
use crate::model::enums::{AvatarStyle, UserRank};
use crate::schema::{user, user_token};
use crate::time::DateTime;
use diesel::pg::Pg;
use diesel::prelude::*;
use std::option::Option;
use uuid::Uuid;

#[derive(Insertable)]
#[diesel(table_name = user)]
#[diesel(check_for_backend(Pg))]
pub struct NewUser<'a> {
    pub name: &'a str,
    pub password_hash: &'a str,
    pub password_salt: &'a str,
    pub email: Option<&'a str>,
    pub rank: UserRank,
    pub avatar_style: AvatarStyle,
}

#[derive(Identifiable, Queryable, Selectable)]
#[diesel(table_name = user)]
#[diesel(check_for_backend(Pg))]
pub struct User {
    pub id: i32,
    pub name: String,
    pub password_hash: String,
    pub password_salt: String,
    pub email: Option<String>,
    pub rank: UserRank,
    pub avatar_style: AvatarStyle,
    pub creation_time: DateTime,
    pub last_login_time: DateTime,
    pub last_edit_time: DateTime,
    #[allow(dead_code)]
    custom_avatar_size: i64,
}

impl User {
    pub fn from_name(conn: &mut PgConnection, name: &str) -> QueryResult<Self> {
        user::table.filter(user::name.eq(name)).first(conn)
    }

    pub fn avatar_url(&self) -> String {
        match self.avatar_style {
            AvatarStyle::Gravatar => hash::gravatar_url(&self.name),
            AvatarStyle::Manual => hash::custom_avatar_url(&self.name),
        }
    }
}

#[derive(Insertable)]
#[diesel(table_name = user_token)]
#[diesel(check_for_backend(Pg))]
pub struct NewUserToken<'a> {
    pub user_id: i32,
    pub token: Uuid,
    pub note: Option<&'a str>,
    pub enabled: bool,
    pub expiration_time: Option<DateTime>,
}

#[derive(Associations, Identifiable, Queryable, Selectable)]
#[diesel(belongs_to(User))]
#[diesel(table_name = user_token)]
#[diesel(primary_key(user_id))]
#[diesel(check_for_backend(Pg))]
pub struct UserToken {
    pub user_id: i32,
    pub token: Uuid,
    pub note: String,
    pub enabled: bool,
    pub expiration_time: Option<DateTime>,
    pub creation_time: DateTime,
    pub last_edit_time: DateTime,
    pub last_usage_time: DateTime,
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::test::*;

    #[test]
    fn save_user() {
        let user = test_transaction(|conn: &mut PgConnection| create_test_user(conn, TEST_USERNAME, UserRank::Regular));

        assert_eq!(user.name, TEST_USERNAME);
        assert_eq!(user.password_hash, TEST_HASH);
        assert_eq!(user.password_salt, TEST_SALT);
        assert_eq!(user.rank, TEST_PRIVILEGE);
    }

    #[test]
    fn save_user_token() {
        let user_token = test_transaction(|conn: &mut PgConnection| {
            create_test_user(conn, TEST_USERNAME, UserRank::Regular)
                .and_then(|user| create_test_user_token(conn, &user, false, None))
        });

        assert_eq!(user_token.token, TEST_TOKEN);
        assert_eq!(user_token.note, "");
        assert!(!user_token.enabled);
        assert_eq!(user_token.expiration_time, None);
    }
}
