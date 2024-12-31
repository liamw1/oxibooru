use crate::content::hash::PostHash;
use crate::model::pool::{Pool, PoolName, PoolPost};
use crate::resource;
use crate::resource::post::MicroPost;
use crate::schema::{pool, pool_category, pool_name, pool_post};
use crate::time::DateTime;
use diesel::dsl::*;
use diesel::prelude::*;
use serde::Serialize;
use serde_with::skip_serializing_none;
use std::str::FromStr;
use strum::{EnumString, EnumTable};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MicroPool {
    pub id: i32,
    pub names: Vec<String>,
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
            .map(Self::from_str)
            .collect::<Result<Vec<_>, _>>()?;
        for field in fields.into_iter() {
            table[field] = true;
        }
        Ok(table)
    }
}

#[skip_serializing_none]
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

    pub fn new_batch(conn: &mut PgConnection, pools: Vec<Pool>, fields: &FieldTable<bool>) -> QueryResult<Vec<Self>> {
        let batch_size = pools.len();

        let mut categories = fields[Field::Category]
            .then(|| get_categories(conn, &pools))
            .transpose()?
            .unwrap_or_default();
        resource::check_batch_results(categories.len(), batch_size);

        let mut names = fields[Field::Names]
            .then(|| get_names(conn, &pools))
            .transpose()?
            .unwrap_or_default();
        resource::check_batch_results(names.len(), batch_size);

        let mut posts = fields[Field::Posts]
            .then(|| get_posts(conn, &pools))
            .transpose()?
            .unwrap_or_default();
        resource::check_batch_results(posts.len(), batch_size);

        let mut post_counts = fields[Field::PostCount]
            .then(|| get_post_counts(conn, &pools))
            .transpose()?
            .unwrap_or_default();
        resource::check_batch_results(post_counts.len(), batch_size);

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
        pool_ids: Vec<i32>,
        fields: &FieldTable<bool>,
    ) -> QueryResult<Vec<Self>> {
        let unordered_pools = pool::table.filter(pool::id.eq_any(&pool_ids)).load(conn)?;
        let pools = resource::order_as(unordered_pools, &pool_ids);
        Self::new_batch(conn, pools, fields)
    }
}

fn get_categories(conn: &mut PgConnection, pools: &[Pool]) -> QueryResult<Vec<String>> {
    let pool_ids: Vec<_> = pools.iter().map(|pool| pool.id).collect();
    pool::table
        .inner_join(pool_category::table)
        .select((pool::id, pool_category::name))
        .filter(pool::id.eq_any(&pool_ids))
        .load(conn)
        .map(|category_names| {
            resource::order_transformed_as(category_names, &pool_ids, |(pool_id, _)| *pool_id)
                .into_iter()
                .map(|(_, category_name)| category_name)
                .collect()
        })
}

fn get_names(conn: &mut PgConnection, pools: &[Pool]) -> QueryResult<Vec<Vec<String>>> {
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
                .map(|pool_post| MicroPost {
                    id: pool_post.post_id,
                    thumbnail_url: PostHash::new(pool_post.post_id).thumbnail_url(),
                })
                .collect()
        })
        .collect())
}

fn get_post_counts(conn: &mut PgConnection, pools: &[Pool]) -> QueryResult<Vec<i64>> {
    PoolPost::belonging_to(pools)
        .group_by(pool_post::pool_id)
        .select((pool_post::pool_id, count(pool_post::post_id)))
        .load(conn)
        .map(|usages| {
            resource::order_like(usages, pools, |(id, _)| *id)
                .into_iter()
                .map(|post_count| post_count.map(|(_, count)| count).unwrap_or(0))
                .collect()
        })
}
