use crate::auth::content;
use crate::model::pool::{Pool, PoolId, PoolName, PoolPost};
use crate::resource;
use crate::resource::post::MicroPost;
use crate::schema::{pool, pool_category, pool_post};
use crate::util::DateTime;
use diesel::dsl::*;
use diesel::prelude::*;
use serde::Serialize;
use std::collections::HashMap;
use std::str::FromStr;
use strum::{EnumString, EnumTable};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MicroPool {
    pub id: i32,
    pub names: Vec<PoolName>,
    pub category: String,
    pub description: String,
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

impl Field {
    pub fn create_table(fields_str: &str) -> Result<FieldTable<bool>, <Self as FromStr>::Err> {
        let mut table = FieldTable::filled(false);
        let fields = fields_str
            .split(',')
            .into_iter()
            .map(Self::from_str)
            .collect::<Result<Vec<_>, _>>()?;
        for field in fields.into_iter() {
            table[field] = true;
        }
        Ok(table)
    }
}

// TODO: Remove renames by changing references to these names in client
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PoolInfo {
    version: Option<DateTime>,
    id: Option<i32>,
    description: Option<String>,
    creation_time: Option<DateTime>,
    last_edit_time: Option<DateTime>,
    category: Option<String>,
    names: Option<Vec<String>>,
    posts: Option<Vec<MicroPost>>,
    post_count: Option<i64>,
}

impl PoolInfo {
    pub fn new(conn: &mut PgConnection, pool: Pool, fields: &FieldTable<bool>) -> QueryResult<Self> {
        let mut pool_info = Self::new_batch(conn, vec![pool], fields)?;
        assert_eq!(pool_info.len(), 1);
        Ok(pool_info.pop().unwrap())
    }

    pub fn new_from_id(conn: &mut PgConnection, pool_id: i32, fields: &FieldTable<bool>) -> QueryResult<Self> {
        let mut pool_info = Self::new_batch_from_ids(conn, vec![pool_id], fields)?;
        assert_eq!(pool_info.len(), 1);
        Ok(pool_info.pop().unwrap())
    }

    pub fn new_batch(
        conn: &mut PgConnection,
        mut pools: Vec<Pool>,
        fields: &FieldTable<bool>,
    ) -> QueryResult<Vec<Self>> {
        let batch_size = pools.len();

        let mut categories = fields[Field::Category]
            .then_some(get_categories(conn, &pools)?)
            .unwrap_or_default();
        resource::check_batch_results(categories.len(), batch_size);

        let mut names = fields[Field::Names]
            .then_some(get_names(conn, &pools)?)
            .unwrap_or_default();
        resource::check_batch_results(names.len(), batch_size);

        let mut posts = fields[Field::Posts]
            .then_some(get_posts(conn, &pools)?)
            .unwrap_or_default();
        resource::check_batch_results(posts.len(), batch_size);

        let mut post_counts = fields[Field::PostCount]
            .then_some(get_post_counts(conn, &pools)?)
            .unwrap_or_default();
        resource::check_batch_results(post_counts.len(), batch_size);

        let mut results: Vec<Self> = Vec::new();
        while let Some(pool) = pools.pop() {
            results.push(Self {
                version: fields[Field::Version].then_some(pool.last_edit_time),
                id: fields[Field::Id].then_some(pool.id),
                description: fields[Field::Description].then_some(pool.description),
                creation_time: fields[Field::CreationTime].then_some(pool.creation_time),
                last_edit_time: fields[Field::LastEditTime].then_some(pool.last_edit_time),
                category: categories.pop(),
                names: names.pop(),
                posts: posts.pop(),
                post_count: post_counts.pop(),
            });
        }
        Ok(results.into_iter().rev().collect())
    }

    pub fn new_batch_from_ids(
        conn: &mut PgConnection,
        pool_ids: Vec<i32>,
        fields: &FieldTable<bool>,
    ) -> QueryResult<Vec<Self>> {
        let pools = get_pools(conn, &pool_ids)?;
        Self::new_batch(conn, pools, fields)
    }
}

fn get_pools(conn: &mut PgConnection, pool_ids: &[i32]) -> QueryResult<Vec<Pool>> {
    // We get pools here, but this query doesn't preserve order
    let mut pools = pool::table
        .select(Pool::as_select())
        .filter(pool::id.eq_any(pool_ids))
        .load(conn)?;

    /*
        This algorithm is O(n^2) in pool_ids.len(), which could be made O(n) with a HashMap implementation.
        However, for small n this Vec-based implementation is probably much faster. Since we retrieve
        40-50 pools at a time, I'm leaving it like this for the time being until it proves to be slow.
    */
    let mut index = 0;
    while index < pool_ids.len() {
        let pool_id = pools[index].id;
        let correct_index = pool_ids.iter().position(|&id| id == pool_id).unwrap();
        if index != correct_index {
            pools.swap(index, correct_index);
        } else {
            index += 1;
        }
    }

    Ok(pools)
}

fn get_categories(conn: &mut PgConnection, pools: &[Pool]) -> QueryResult<Vec<String>> {
    let category_names: HashMap<i32, String> = pool_category::table
        .select((pool_category::id, pool_category::name))
        .load(conn)?
        .into_iter()
        .collect();
    Ok(pools.iter().map(|pool| category_names[&pool.id].clone()).collect())
}

fn get_names(conn: &mut PgConnection, pools: &[Pool]) -> QueryResult<Vec<Vec<String>>> {
    let names = PoolName::belonging_to(pools).select(PoolName::as_select()).load(conn)?;
    Ok(names
        .grouped_by(pools)
        .into_iter()
        .map(|pool_names| pool_names.into_iter().map(|pool_name| pool_name.name).collect())
        .collect())
}

fn get_posts(conn: &mut PgConnection, pools: &[Pool]) -> QueryResult<Vec<Vec<MicroPost>>> {
    let pool_posts: Vec<PoolPost> = PoolPost::belonging_to(pools).select(PoolPost::as_select()).load(conn)?;
    Ok(pool_posts
        .grouped_by(pools)
        .into_iter()
        .map(|posts_in_pool| {
            posts_in_pool
                .into_iter()
                .map(|pool_post| MicroPost {
                    id: pool_post.post_id,
                    thumbnail_url: content::post_thumbnail_url(pool_post.post_id),
                })
                .collect()
        })
        .collect())
}

fn get_post_counts(conn: &mut PgConnection, pools: &[Pool]) -> QueryResult<Vec<i64>> {
    let usages: Vec<(PoolId, i64)> = PoolPost::belonging_to(pools)
        .group_by(pool_post::pool_id)
        .select((pool_post::pool_id, count(pool_post::post_id)))
        .load(conn)?;
    Ok(usages
        .grouped_by(pools)
        .into_iter()
        .map(|counts| counts.first().map(|(_, count)| *count).unwrap_or(0))
        .collect())
}
