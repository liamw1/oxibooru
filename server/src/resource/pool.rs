use crate::model::pool::{Pool, PoolName, PoolPost};
use crate::resource::post::MicroPost;
use crate::resource::{self, BoolFill};
use crate::schema::{pool, pool_category, pool_name, pool_post, pool_statistics};
use crate::string::{LargeString, SmallString};
use crate::time::DateTime;
use diesel::prelude::*;
use serde::Serialize;
use serde_with::skip_serializing_none;
use std::collections::HashMap;
use std::rc::Rc;
use strum::{EnumString, EnumTable};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MicroPool {
    pub id: i64,
    pub names: Rc<[SmallString]>,
    pub category: SmallString,
    pub description: LargeString,
    pub post_count: i64,
}

#[derive(Clone, Copy, EnumString, EnumTable)]
#[strum(serialize_all = "camelCase")]
pub enum Field {
    Version,
    Id,
    Description,
    CreationTime,
    LastEditTime,
    Category,
    Names,
    Posts,
    PostCount,
}

impl BoolFill for FieldTable<bool> {
    fn filled(val: bool) -> Self {
        Self::filled(val)
    }
}

#[skip_serializing_none]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PoolInfo {
    version: Option<DateTime>,
    id: Option<i64>,
    description: Option<LargeString>,
    creation_time: Option<DateTime>,
    last_edit_time: Option<DateTime>,
    category: Option<SmallString>,
    names: Option<Vec<SmallString>>,
    posts: Option<Vec<MicroPost>>,
    post_count: Option<i64>,
}

impl PoolInfo {
    pub fn new(conn: &mut PgConnection, pool: Pool, fields: &FieldTable<bool>) -> QueryResult<Self> {
        Self::new_batch(conn, vec![pool], fields).map(resource::single)
    }

    pub fn new_from_id(conn: &mut PgConnection, pool_id: i64, fields: &FieldTable<bool>) -> QueryResult<Self> {
        Self::new_batch_from_ids(conn, &[pool_id], fields).map(resource::single)
    }

    pub fn new_batch(conn: &mut PgConnection, pools: Vec<Pool>, fields: &FieldTable<bool>) -> QueryResult<Vec<Self>> {
        let mut categories = resource::retrieve(fields[Field::Category], || get_categories(conn, &pools))?;
        let mut names = resource::retrieve(fields[Field::Names], || get_names(conn, &pools))?;
        let mut posts = resource::retrieve(fields[Field::Posts], || get_posts(conn, &pools))?;
        let mut post_counts = resource::retrieve(fields[Field::PostCount], || get_post_counts(conn, &pools))?;

        let batch_size = pools.len();
        resource::check_batch_results(batch_size, names.len());
        resource::check_batch_results(batch_size, categories.len());
        resource::check_batch_results(batch_size, posts.len());
        resource::check_batch_results(batch_size, post_counts.len());

        let results = pools
            .into_iter()
            .rev()
            .map(|pool| Self {
                version: fields[Field::Version].then_some(pool.last_edit_time),
                id: fields[Field::Id].then_some(pool.id),
                description: fields[Field::Description].then_some(pool.description),
                creation_time: fields[Field::CreationTime].then_some(pool.creation_time),
                last_edit_time: fields[Field::LastEditTime].then_some(pool.last_edit_time),
                category: categories.pop(),
                names: names.pop(),
                posts: posts.pop(),
                post_count: post_counts.pop(),
            })
            .collect::<Vec<_>>();
        Ok(results.into_iter().rev().collect())
    }

    pub fn new_batch_from_ids(
        conn: &mut PgConnection,
        pool_ids: &[i64],
        fields: &FieldTable<bool>,
    ) -> QueryResult<Vec<Self>> {
        let unordered_pools = pool::table.filter(pool::id.eq_any(pool_ids)).load(conn)?;
        let pools = resource::order_as(unordered_pools, pool_ids);
        Self::new_batch(conn, pools, fields)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PoolNeighborInfo {
    pool: MicroPool,
    first_post: MicroPost,
    last_post: MicroPost,
    previous_post: Option<MicroPost>,
    next_post: Option<MicroPost>,
}

impl PoolNeighborInfo {
    pub fn retrieve(conn: &mut PgConnection, post_id: i64) -> QueryResult<Vec<PoolNeighborInfo>> {
        let pools = pool::table
            .inner_join(pool_post::table)
            .select(Pool::as_select())
            .filter(pool_post::post_id.eq(post_id))
            .order(pool::id)
            .load(conn)?;

        let category_names: HashMap<i64, SmallString> = pool_category::table
            .select((pool_category::id, pool_category::name))
            .load(conn)?
            .into_iter()
            .collect();

        let pool_names = get_names(conn, &pools)?;
        let post_counts = get_post_counts(conn, &pools)?;
        let first_posts = get_first_posts_in_pools(conn, &pools)?;
        let last_posts = get_last_posts_in_pools(conn, &pools)?;
        let prev_posts = get_prev_posts_in_pools(conn, post_id, &pools)?;
        let next_posts = get_next_posts_in_pools(conn, post_id, &pools)?;

        let batch_size = pools.len();
        resource::check_batch_results(batch_size, pool_names.len());
        resource::check_batch_results(batch_size, post_counts.len());
        resource::check_batch_results(batch_size, first_posts.len());
        resource::check_batch_results(batch_size, last_posts.len());
        resource::check_batch_results(batch_size, prev_posts.len());
        resource::check_batch_results(batch_size, next_posts.len());

        let results = pools
            .into_iter()
            .zip(pool_names)
            .zip(post_counts)
            .zip(first_posts)
            .zip(last_posts)
            .zip(prev_posts)
            .zip(next_posts)
            .rev()
            .map(|((((((pool, names), post_count), first_post), last_post), previous_post), next_post)| {
                let micro_pool = MicroPool {
                    id: pool.id,
                    names: names.into(),
                    category: category_names[&pool.category_id].clone(),
                    description: pool.description,
                    post_count,
                };
                Self {
                    pool: micro_pool,
                    first_post,
                    last_post,
                    previous_post,
                    next_post,
                }
            })
            .collect::<Vec<_>>();
        Ok(results.into_iter().rev().collect())
    }
}

fn get_categories(conn: &mut PgConnection, pools: &[Pool]) -> QueryResult<Vec<SmallString>> {
    let pool_ids: Vec<_> = pools.iter().map(Identifiable::id).copied().collect();
    pool::table
        .inner_join(pool_category::table)
        .select((pool::id, pool_category::name))
        .filter(pool::id.eq_any(&pool_ids))
        .load(conn)
        .map(|category_names| {
            resource::order_transformed_as(category_names, &pool_ids, |&(pool_id, _)| pool_id)
                .into_iter()
                .map(|(_, category_name)| category_name)
                .collect()
        })
}

fn get_names(conn: &mut PgConnection, pools: &[Pool]) -> QueryResult<Vec<Vec<SmallString>>> {
    Ok(PoolName::belonging_to(pools)
        .order_by(pool_name::order)
        .load::<PoolName>(conn)?
        .grouped_by(pools)
        .into_iter()
        .map(|pool_names| pool_names.into_iter().map(|pool_name| pool_name.name).collect())
        .collect())
}

fn get_posts(conn: &mut PgConnection, pools: &[Pool]) -> QueryResult<Vec<Vec<MicroPost>>> {
    Ok(PoolPost::belonging_to(pools)
        .order_by(pool_post::order)
        .load::<PoolPost>(conn)?
        .grouped_by(pools)
        .into_iter()
        .map(|posts_in_pool| {
            posts_in_pool
                .into_iter()
                .map(|pool_post| pool_post.post_id)
                .map(MicroPost::new)
                .collect()
        })
        .collect())
}

fn get_post_counts(conn: &mut PgConnection, pools: &[Pool]) -> QueryResult<Vec<i64>> {
    let pool_ids: Vec<_> = pools.iter().map(Identifiable::id).copied().collect();
    pool_statistics::table
        .select((pool_statistics::pool_id, pool_statistics::post_count))
        .filter(pool_statistics::pool_id.eq_any(&pool_ids))
        .load(conn)
        .map(|usages| {
            resource::order_transformed_as(usages, &pool_ids, |&(id, _)| id)
                .into_iter()
                .map(|(_, post_count)| post_count)
                .collect()
        })
}

fn get_first_posts_in_pools(conn: &mut PgConnection, pools: &[Pool]) -> QueryResult<Vec<MicroPost>> {
    PoolPost::belonging_to(pools)
        .select(pool_post::post_id)
        .distinct_on(pool_post::pool_id)
        .order_by((pool_post::pool_id, pool_post::order))
        .load(conn)
        .map(|post_ids| post_ids.into_iter().map(MicroPost::new).collect())
}

fn get_last_posts_in_pools(conn: &mut PgConnection, pools: &[Pool]) -> QueryResult<Vec<MicroPost>> {
    PoolPost::belonging_to(pools)
        .select(pool_post::post_id)
        .distinct_on(pool_post::pool_id)
        .order_by((pool_post::pool_id, pool_post::order.desc()))
        .load(conn)
        .map(|post_ids| post_ids.into_iter().map(MicroPost::new).collect())
}

fn get_prev_posts_in_pools(
    conn: &mut PgConnection,
    post_id: i64,
    pools: &[Pool],
) -> QueryResult<Vec<Option<MicroPost>>> {
    diesel::alias!(pool_post as current_post: CurrentPost);

    PoolPost::belonging_to(pools)
        .inner_join(
            current_post.on(pool_post::pool_id
                .eq(current_post.field(pool_post::pool_id))
                .and(current_post.field(pool_post::post_id).eq(post_id))),
        )
        .select(PoolPost::as_select())
        .distinct_on(pool_post::pool_id)
        .filter(pool_post::order.lt(current_post.field(pool_post::order)))
        .order_by((pool_post::pool_id, pool_post::order.desc()))
        .load(conn)
        .map(|prev_posts| {
            resource::order_like(prev_posts, &pools, |pool_post| pool_post.pool_id)
                .into_iter()
                .map(|pool_post| pool_post.map(|pool_post| MicroPost::new(pool_post.post_id)))
                .collect()
        })
}

fn get_next_posts_in_pools(
    conn: &mut PgConnection,
    post_id: i64,
    pools: &[Pool],
) -> QueryResult<Vec<Option<MicroPost>>> {
    diesel::alias!(pool_post as current_post: CurrentPost);

    PoolPost::belonging_to(pools)
        .inner_join(
            current_post.on(pool_post::pool_id
                .eq(current_post.field(pool_post::pool_id))
                .and(current_post.field(pool_post::post_id).eq(post_id))),
        )
        .select(PoolPost::as_select())
        .distinct_on(pool_post::pool_id)
        .filter(pool_post::order.gt(current_post.field(pool_post::order)))
        .order_by((pool_post::pool_id, pool_post::order))
        .load(conn)
        .map(|prev_posts| {
            resource::order_like(prev_posts, &pools, |pool_post| pool_post.pool_id)
                .into_iter()
                .map(|pool_post| pool_post.map(|pool_post| MicroPost::new(pool_post.post_id)))
                .collect()
        })
}
