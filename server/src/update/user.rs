use crate::api::error::ApiResult;
use crate::config::Config;
use crate::filesystem;
use crate::schema::user;
use crate::time::DateTime;
use diesel::{ExpressionMethods, PgConnection, QueryDsl, QueryResult, RunQueryDsl};
use image::DynamicImage;

/// Updates the last known login time for the user with the given `user_id`.
pub fn last_login_time(conn: &mut PgConnection, user_id: i64) -> QueryResult<()> {
    diesel::update(user::table.find(user_id))
        .set(user::last_login_time.eq(DateTime::now()))
        .execute(conn)?;
    Ok(())
}

/// Updates `last_edit_time` of user with given `user_id`.
pub fn last_edit_time(conn: &mut PgConnection, user_id: i64) -> QueryResult<()> {
    diesel::update(user::table.find(user_id))
        .set(user::last_edit_time.eq(DateTime::now()))
        .execute(conn)?;
    Ok(())
}

/// Updates custom avatar for user.
pub fn avatar(
    conn: &mut PgConnection,
    config: &Config,
    user_id: i64,
    name: &str,
    avatar: &DynamicImage,
) -> ApiResult<()> {
    filesystem::delete_custom_avatar(config, name)?;

    let avatar_size = filesystem::save_custom_avatar(config, name, avatar)?;
    diesel::update(user::table.find(user_id))
        .set(user::custom_avatar_size.eq(avatar_size))
        .execute(conn)?;
    Ok(())
}
