use crate::api::error::{self, ApiError, ApiResult};
use crate::app::Context;
use crate::config::{Action, Config, RegexType};
use crate::model::enums::{ResourceProperty, ResourceType};
use crate::model::pool::{NewPool, NewPoolName, PoolPost};
use crate::schema::{pool, pool_name, pool_post};
use crate::string::SmallString;
use crate::time::DateTime;
use crate::update::NameType;
use crate::{api, snapshot, update};
use diesel::dsl::{exists, max};
use diesel::{ExpressionMethods, Insertable, PgConnection, QueryDsl, QueryResult, RunQueryDsl};

/// Updates `last_edit_time` of pool associated with `pool_id`.
pub fn last_edit_time(conn: &mut PgConnection, pool_id: i64) -> QueryResult<()> {
    diesel::update(pool::table.find(pool_id))
        .set(pool::last_edit_time.eq(DateTime::now()))
        .execute(conn)?;
    Ok(())
}

/// Returns all pool ids implied from the given set of names.
/// Returned ids will be distinct.
///
/// Requires pool creation privileges if new names are given.
/// Checks that each new name matches on the Pool regex.
pub fn get_or_create_pools(conn: &mut PgConnection, ctx: &Context, names: Vec<SmallString>) -> ApiResult<Vec<i64>> {
    let (mut pool_ids, new_names) = fetch_pools(conn, ctx, names)?;

    // Create new pools if given unique names
    if !new_names.is_empty() {
        ctx.verify_privilege(Action::PoolCreate)?;

        let new_pool_ids: Vec<i64> = vec![NewPool::default(); new_names.len()]
            .insert_into(pool::table)
            .returning(pool::id)
            .get_results(conn)?;
        let new_pool_names: Vec<_> = new_pool_ids
            .iter()
            .zip(new_names.iter())
            .map(|(&pool_id, name)| NewPoolName {
                pool_id,
                order: 0,
                name,
            })
            .collect();
        new_pool_names.insert_into(pool_name::table).execute(conn)?;

        snapshot::pool::new_name_snapshots(conn, ctx.client, new_names)?;
        pool_ids.extend(new_pool_ids);
    }
    Ok(pool_ids)
}

pub fn fetch_pools(
    conn: &mut PgConnection,
    ctx: &Context,
    names: Vec<SmallString>,
) -> ApiResult<(Vec<i64>, Vec<SmallString>)> {
    let pool_ids: Vec<i64> = pool_name::table
        .select(pool_name::pool_id)
        .filter(pool_name::name.eq_any(&names))
        .distinct()
        .load(conn)?;

    let new_names = update::get_new_names(conn, &names, NameType::Pool)?;
    new_names
        .iter()
        .try_for_each(|name| api::verify_matches_regex(&ctx.config, name, RegexType::Pool))?;
    Ok((pool_ids, new_names))
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
pub fn set_posts(conn: &mut PgConnection, ctx: &Context, pool_id: i64, posts: &mut Vec<i64>) -> ApiResult<()> {
    // Add posts client doesn't know about
    if let Some(hidden_posts) = ctx.preferences().hidden_posts(pool_post::post_id) {
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
        .order(pool_post::order)
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
