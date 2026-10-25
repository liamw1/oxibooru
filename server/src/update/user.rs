use crate::api::error::ApiResult;
use crate::config::Config;
use crate::filesystem;
use crate::model::user::UserTagBlocklist;
use crate::schema::{user, user_tag_blocklist};
use crate::time::DateTime;
use diesel::{ExpressionMethods, Insertable, PgConnection, QueryDsl, QueryResult, RunQueryDsl};
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
    lowercase_username: &str,
    avatar: DynamicImage,
) -> ApiResult<()> {
    filesystem::delete_custom_avatar(config, lowercase_username)?;

    let avatar_size = filesystem::save_custom_avatar(config, lowercase_username, avatar)?;
    diesel::update(user::table.find(user_id))
        .set(user::custom_avatar_size.eq(avatar_size))
        .execute(conn)?;
    Ok(())
}

/// Replaces the current set of tags of blocklist with new ones.
pub fn set_tags_blocklist(conn: &mut PgConnection, user_id: i64, tags: &[i64]) -> QueryResult<()> {
    let new_blocklist_tags: Vec<_> = tags
        .iter()
        .map(|&tag_id| UserTagBlocklist { user_id, tag_id })
        .collect();

    diesel::delete(user_tag_blocklist::table)
        .filter(user_tag_blocklist::user_id.eq(user_id))
        .execute(conn)?;
    new_blocklist_tags
        .insert_into(user_tag_blocklist::table)
        .execute(conn)?;
    Ok(())
}
