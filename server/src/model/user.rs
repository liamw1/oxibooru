use crate::auth::content;
use crate::model::enums::{AvatarStyle, UserRank};
use crate::schema::{user, user_token};
use crate::util::DateTime;
use diesel::deserialize::{self, FromSql, FromSqlRow};
use diesel::expression::AsExpression;
use diesel::pg::Pg;
use diesel::prelude::*;
use diesel::serialize::{self, Output, ToSql};
use diesel::sql_types::Integer;
use std::option::Option;
use uuid::Uuid;

#[derive(Insertable)]
#[diesel(table_name = user)]
pub struct NewUser<'a> {
    pub name: &'a str,
    pub password_hash: &'a str,
    pub password_salt: &'a str,
    pub email: Option<&'a str>,
    pub rank: UserRank,
    pub avatar_style: AvatarStyle,
}

#[derive(Debug, Clone, Copy, Associations, Selectable, AsExpression, FromSqlRow)]
#[diesel(sql_type = Integer)]
#[diesel(belongs_to(User, foreign_key = id))]
#[diesel(table_name = user)]
#[diesel(check_for_backend(Pg))]
pub struct UserId {
    pub id: i32,
}

impl ToSql<Integer, Pg> for UserId
where
    i32: ToSql<Integer, Pg>,
{
    fn to_sql(&self, out: &mut Output<Pg>) -> serialize::Result {
        <i32 as ToSql<Integer, Pg>>::to_sql(&self.id, &mut out.reborrow())
    }
}

impl FromSql<Integer, Pg> for UserId
where
    i32: FromSql<Integer, Pg>,
{
    fn from_sql(bytes: <Pg as diesel::backend::Backend>::RawValue<'_>) -> deserialize::Result<Self> {
        i32::from_sql(bytes).map(|id| UserId { id })
    }
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
    pub avatar_style: AvatarStyle,
    pub rank: UserRank,
    pub creation_time: DateTime,
    pub last_login_time: DateTime,
    pub last_edit_time: DateTime,
}

impl User {
    pub fn from_name(conn: &mut PgConnection, name: &str) -> QueryResult<Self> {
        user::table
            .select(Self::as_select())
            .filter(user::name.eq(name))
            .first(conn)
    }

    pub fn avatar_url(&self) -> String {
        match self.avatar_style {
            AvatarStyle::Gravatar => content::gravatar_url(&self.name),
            AvatarStyle::Manual => content::custom_avatar_url(&self.name),
        }
    }
}

#[derive(Insertable)]
#[diesel(table_name = user_token)]
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
    pub note: Option<String>,
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
        let user = test_transaction(|conn: &mut PgConnection| create_test_user(conn, TEST_USERNAME));

        assert_eq!(user.name, TEST_USERNAME);
        assert_eq!(user.password_hash, TEST_HASH);
        assert_eq!(user.password_salt, TEST_SALT);
        assert_eq!(user.rank, TEST_PRIVILEGE);
    }

    #[test]
    fn save_user_token() {
        let user_token = test_transaction(|conn: &mut PgConnection| {
            create_test_user(conn, "test_user").and_then(|user| create_test_user_token(conn, &user, false, None))
        });

        assert_eq!(user_token.token, TEST_TOKEN);
        assert_eq!(user_token.note, None);
        assert!(!user_token.enabled);
        assert_eq!(user_token.expiration_time, None);
    }
}
