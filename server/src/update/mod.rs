pub mod pool;
pub mod post;
pub mod tag;

use crate::api::ApiResult;
use crate::db;
use crate::model::user::User;
use crate::schema::user;
use crate::time::DateTime;
use diesel::prelude::*;

// NOTE: Unless otherwise stated, the functions in this module do not check that the
// client has the required privileges to perform their respective actions.
// Make sure to check for privileges before calling them, if necessary.

/// Retrieves the last known login time for the given `user`.
pub fn last_login_time(user: &User) -> ApiResult<()> {
    let mut conn = db::get_connection()?;
    diesel::update(user::table.find(user.id))
        .set(user::last_login_time.eq(DateTime::now()))
        .execute(&mut conn)?;
    Ok(())
}
