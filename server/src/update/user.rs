use crate::api::ApiResult;
use crate::schema::user;
use crate::time::DateTime;
use crate::{db, filesystem};
use diesel::prelude::*;
use image::DynamicImage;

/// Updates the last known login time for the user with the given `user_id`.
pub fn last_login_time(user_id: i64) -> ApiResult<()> {
    let mut conn = db::get_connection()?;
    diesel::update(user::table.find(user_id))
        .set(user::last_login_time.eq(DateTime::now()))
        .execute(&mut conn)?;
    Ok(())
}

/// Updates `last_edit_time` of user with given `user_id`.
pub fn last_edit_time(conn: &mut PgConnection, user_id: i64) -> ApiResult<()> {
    diesel::update(user::table.find(user_id))
        .set(user::last_edit_time.eq(DateTime::now()))
        .execute(conn)?;
    Ok(())
}

/// Updates custom avatar for user.
pub fn avatar(conn: &mut PgConnection, user_id: i64, name: &str, avatar: &DynamicImage) -> ApiResult<()> {
    filesystem::delete_custom_avatar(name)?;

    let avatar_size = filesystem::save_custom_avatar(name, avatar)?;
    diesel::update(user::table.find(user_id))
        .set(user::custom_avatar_size.eq(avatar_size as i64))
        .execute(conn)?;
    Ok(())
}
