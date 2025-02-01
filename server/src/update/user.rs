use crate::api::ApiResult;
use crate::auth::header::AuthUser;
use crate::schema::user;
use crate::time::DateTime;
use crate::{api, config, db, filesystem};
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

pub fn avatar(
    conn: &mut PgConnection,
    client: Option<AuthUser>,
    user_id: i64,
    name: &str,
    avatar: DynamicImage,
    updating_self: bool,
) -> ApiResult<()> {
    let required_rank = match updating_self {
        true => config::privileges().user_edit_self_avatar,
        false => config::privileges().user_edit_any_avatar,
    };
    api::verify_privilege(client, required_rank)?;

    filesystem::delete_custom_avatar(name)?;

    let avatar_size = filesystem::save_custom_avatar(name, avatar)?;
    diesel::update(user::table.find(user_id))
        .set(user::custom_avatar_size.eq(avatar_size as i64))
        .execute(conn)?;
    Ok(())
}
