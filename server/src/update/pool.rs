use crate::api;
use crate::api::ApiResult;
use crate::config::RegexType;
use crate::model::pool::{NewPoolName, PoolPost};
use crate::schema::{pool, pool_name, pool_post};
use crate::string::SmallString;
use crate::time::DateTime;
use diesel::prelude::*;

/// Updates last_edit_time of pool with given `pool_id`.
pub fn last_edit_time(conn: &mut PgConnection, pool_id: i64) -> ApiResult<()> {
    diesel::update(pool::table.find(pool_id))
        .set(pool::last_edit_time.eq(DateTime::now()))
        .execute(conn)?;
    Ok(())
}

/// Appends `names` onto the current list of names for the pool with id `pool_id`.
pub fn add_names(
    conn: &mut PgConnection,
    pool_id: i64,
    current_name_count: i32,
    names: Vec<SmallString>,
) -> ApiResult<()> {
    names
        .iter()
        .try_for_each(|name| api::verify_matches_regex(name, RegexType::Pool))?;

    let updated_names: Vec<_> = names
        .iter()
        .enumerate()
        .map(|(i, name)| (current_name_count + i as i32, name))
        .map(|(order, name)| NewPoolName { pool_id, order, name })
        .collect();
    updated_names.insert_into(pool_name::table).execute(conn)?;
    Ok(())
}

/// Deletes all names for pool with id `pool_id`.
/// Returns numbers of names deleted.
pub fn delete_names(conn: &mut PgConnection, pool_id: i64) -> QueryResult<usize> {
    diesel::delete(pool_name::table)
        .filter(pool_name::pool_id.eq(pool_id))
        .execute(conn)
}

/// Appends `posts` onto the current list of posts in the pool with id `pool_id`.
pub fn add_posts(conn: &mut PgConnection, pool_id: i64, current_post_count: i64, posts: Vec<i64>) -> QueryResult<()> {
    let new_pool_posts: Vec<_> = posts
        .into_iter()
        .enumerate()
        .map(|(i, post_id)| (current_post_count + i as i64, post_id))
        .map(|(order, post_id)| PoolPost {
            pool_id,
            post_id,
            order,
        })
        .collect();
    new_pool_posts.insert_into(pool_post::table).execute(conn)?;
    Ok(())
}

/// Removes all posts from pool with id `pool_id`.
/// Returns numbers of names removed.
pub fn delete_posts(conn: &mut PgConnection, pool_id: i64) -> QueryResult<usize> {
    diesel::delete(pool_post::table)
        .filter(pool_post::pool_id.eq(pool_id))
        .execute(conn)
}
