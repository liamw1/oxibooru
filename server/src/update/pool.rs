use crate::api::error::{ApiError, ApiResult};
use crate::api::{self, error};
use crate::auth::Client;
use crate::config::{Config, RegexType};
use crate::model::enums::{ResourceProperty, ResourceType};
use crate::model::pool::{NewPoolName, PoolPost};
use crate::schema::{pool, pool_name, pool_post};
use crate::search::preferences::Preferences;
use crate::string::SmallString;
use crate::time::DateTime;
use diesel::dsl::{exists, max};
use diesel::{ExpressionMethods, Insertable, PgConnection, QueryDsl, QueryResult, RunQueryDsl};

/// Updates `last_edit_time` of pool associated with `pool_id`.
pub fn last_edit_time(conn: &mut PgConnection, pool_id: i64) -> QueryResult<()> {
    diesel::update(pool::table.find(pool_id))
        .set(pool::last_edit_time.eq(DateTime::now()))
        .execute(conn)?;
    Ok(())
}

/// Replaces the current ordered list of names with `names` for pool associated with `pool_id`.
pub fn set_names(conn: &mut PgConnection, config: &Config, pool_id: i64, names: &[SmallString]) -> ApiResult<()> {
    names
        .iter()
        .try_for_each(|name| api::verify_matches_regex(config, name, RegexType::Pool))?;

    diesel::delete(pool_name::table)
        .filter(pool_name::pool_id.eq(pool_id))
        .execute(conn)?;
    add_names(conn, pool_id, 0, names)
}

/// Replaces the current ordered list of posts with `posts` for pool associated with `pool_id`.
pub fn set_posts(
    conn: &mut PgConnection,
    config: &Config,
    client: Client,
    pool_id: i64,
    posts: &mut Vec<i64>,
) -> ApiResult<()> {
    // Add posts client doesn't know about
    let preferences = Preferences::new(config, client);
    if let Some(hidden_posts) = preferences.hidden_posts(pool_post::post_id) {
        let hidden_posts: Vec<i64> = pool_post::table
            .select(pool_post::post_id)
            .filter(pool_post::pool_id.eq(pool_id))
            .filter(exists(hidden_posts))
            .load(conn)?;
        posts.extend(hidden_posts);
    }

    diesel::delete(pool_post::table)
        .filter(pool_post::pool_id.eq(pool_id))
        .execute(conn)?;
    add_posts(conn, pool_id, 0, posts)
}

/// Appends `posts` onto the current list of posts in the pool associated with `pool_id`.
pub fn add_posts(conn: &mut PgConnection, pool_id: i64, current_post_count: i64, posts: &[i64]) -> ApiResult<()> {
    let total_post_count = i64::try_from(posts.len())
        .unwrap_or(i64::MAX)
        .saturating_add(current_post_count);
    let new_pool_posts: Vec<_> = posts
        .iter()
        .zip(current_post_count..total_post_count)
        .map(|(&post_id, order)| PoolPost {
            pool_id,
            post_id,
            order,
        })
        .collect();
    let insert_result = new_pool_posts.insert_into(pool_post::table).execute(conn);
    error::map_unique_or_foreign_key_violation(insert_result, ResourceProperty::PoolPost, ResourceType::Post)?;
    Ok(())
}

/// Merges pool associated with `abosorbed_id` to one with associated with `merged_to_id`.
pub fn merge(conn: &mut PgConnection, absorbed_id: i64, merge_to_id: i64) -> ApiResult<()> {
    // Merge posts
    let merge_to_pool_posts = pool_post::table
        .select(pool_post::post_id)
        .filter(pool_post::pool_id.eq(merge_to_id))
        .into_boxed();
    let new_pool_posts: Vec<_> = pool_post::table
        .select(pool_post::post_id)
        .filter(pool_post::pool_id.eq(absorbed_id))
        .filter(pool_post::post_id.ne_all(merge_to_pool_posts))
        .order_by(pool_post::order)
        .load(conn)?;
    let post_count: i64 = pool_post::table
        .filter(pool_post::pool_id.eq(merge_to_id))
        .count()
        .first(conn)?;
    add_posts(conn, merge_to_id, post_count, &new_pool_posts)?;

    // Merge names
    let current_name_count = pool_name::table
        .select(max(pool_name::order) + 1)
        .filter(pool_name::pool_id.eq(merge_to_id))
        .first::<Option<_>>(conn)?
        .unwrap_or(0);
    let removed_names = diesel::delete(pool_name::table.filter(pool_name::pool_id.eq(absorbed_id)))
        .returning(pool_name::name)
        .get_results(conn)?;
    add_names(conn, merge_to_id, current_name_count, &removed_names)?;

    diesel::delete(pool::table.find(absorbed_id)).execute(conn)?;
    last_edit_time(conn, merge_to_id).map_err(ApiError::from)
}

/// Appends `names` onto the current list of names for the pool associated with `pool_id`.
fn add_names(conn: &mut PgConnection, pool_id: i64, current_name_count: i32, names: &[SmallString]) -> ApiResult<()> {
    let total_name_count = i32::try_from(names.len())
        .unwrap_or(i32::MAX)
        .saturating_add(current_name_count);
    let updated_names: Vec<_> = names
        .iter()
        .zip(current_name_count..total_name_count)
        .map(|(name, order)| NewPoolName { pool_id, order, name })
        .collect();
    let insert_result = updated_names.insert_into(pool_name::table).execute(conn);
    error::map_unique_violation(insert_result, ResourceProperty::PoolName)?;
    Ok(())
}
