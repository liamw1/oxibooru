use crate::api;
use crate::api::ApiResult;
use crate::config::RegexType;
use crate::model::pool::{NewPoolName, PoolPost};
use crate::schema::{pool_name, pool_post};
use diesel::prelude::*;

pub fn add_names(conn: &mut PgConnection, pool_id: i32, current_name_count: i32, names: Vec<String>) -> ApiResult<()> {
    names
        .iter()
        .map(|name| api::verify_matches_regex(name, RegexType::Pool))
        .collect::<Result<_, _>>()?;

    let updated_names: Vec<_> = names
        .iter()
        .enumerate()
        .map(|(i, name)| (current_name_count + i as i32, name))
        .map(|(order, name)| NewPoolName { pool_id, order, name })
        .collect();
    diesel::insert_into(pool_name::table)
        .values(updated_names)
        .execute(conn)?;
    Ok(())
}

pub fn delete_names(conn: &mut PgConnection, pool_id: i32) -> QueryResult<usize> {
    diesel::delete(pool_name::table)
        .filter(pool_name::pool_id.eq(pool_id))
        .execute(conn)
}

pub fn add_posts(conn: &mut PgConnection, pool_id: i32, current_post_count: i32, posts: Vec<i32>) -> QueryResult<()> {
    let new_pool_posts: Vec<_> = posts
        .into_iter()
        .enumerate()
        .map(|(i, post_id)| (current_post_count + i as i32, post_id))
        .map(|(order, post_id)| PoolPost {
            pool_id,
            post_id,
            order,
        })
        .collect();
    diesel::insert_into(pool_post::table)
        .values(new_pool_posts)
        .execute(conn)?;
    Ok(())
}

pub fn delete_posts(conn: &mut PgConnection, pool_id: i32) -> QueryResult<usize> {
    diesel::delete(pool_post::table)
        .filter(pool_post::pool_id.eq(pool_id))
        .execute(conn)
}
