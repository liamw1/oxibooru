pub mod pool;
pub mod post;
pub mod tag;

use crate::api::ApiResult;
use crate::db;
use crate::model::user::User;
use crate::schema::user;
use crate::time::DateTime;
use diesel::prelude::*;

pub fn last_login_time(user: &User) -> ApiResult<()> {
    let mut conn = db::get_connection()?;
    diesel::update(user::table.find(user.id))
        .set(user::last_login_time.eq(DateTime::now()))
        .execute(&mut conn)?;
    Ok(())
}
