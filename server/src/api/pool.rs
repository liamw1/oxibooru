use crate::api::{ApiResult, AuthResult, DeleteRequest, MergeRequest, PagedQuery, PagedResponse, ResourceQuery};
use crate::config::RegexType;
use crate::model::pool::{NewPool, Pool};
use crate::resource::pool::{FieldTable, PoolInfo};
use crate::schema::{pool, pool_category, pool_name, pool_post};
use crate::util::DateTime;
use crate::{api, config, resource, search, update};
use diesel::prelude::*;
use serde::Deserialize;
use warp::{Filter, Rejection, Reply};

pub fn routes() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    let list_pools = warp::get()
        .and(warp::path!("pools"))
        .and(api::auth())
        .and(warp::query())
        .map(list_pools)
        .map(api::Reply::from);
    let get_pool = warp::get()
        .and(warp::path!("pool" / i32))
        .and(api::auth())
        .and(api::resource_query())
        .map(get_pool)
        .map(api::Reply::from);
    let create_pool = warp::post()
        .and(warp::path!("pool"))
        .and(api::auth())
        .and(api::resource_query())
        .and(warp::body::json())
        .map(create_pool)
        .map(api::Reply::from);
    let merge_pools = warp::post()
        .and(warp::path!("pool-merge"))
        .and(api::auth())
        .and(api::resource_query())
        .and(warp::body::json())
        .map(merge_pools)
        .map(api::Reply::from);
    let update_pool = warp::put()
        .and(warp::path!("pool" / i32))
        .and(api::auth())
        .and(api::resource_query())
        .and(warp::body::json())
        .map(update_pool)
        .map(api::Reply::from);
    let delete_pool = warp::delete()
        .and(warp::path!("pool" / String))
        .and(api::auth())
        .and(warp::body::json())
        .map(delete_pool)
        .map(api::Reply::from);

    list_pools
        .or(get_pool)
        .or(create_pool)
        .or(merge_pools)
        .or(update_pool)
        .or(delete_pool)
}

type PagedPoolInfo = PagedResponse<PoolInfo>;

const MAX_POOLS_PER_PAGE: i64 = 50;

fn create_field_table(fields: Option<&str>) -> Result<FieldTable<bool>, Box<dyn std::error::Error>> {
    fields
        .map(resource::pool::Field::create_table)
        .transpose()
        .map(|opt_table| opt_table.unwrap_or(FieldTable::filled(true)))
        .map_err(Box::from)
}

fn list_pools(auth: AuthResult, query: PagedQuery) -> ApiResult<PagedPoolInfo> {
    let _timer = crate::util::Timer::new("list_pools");

    let client = auth?;
    api::verify_privilege(client.as_ref(), config::privileges().pool_list)?;

    let offset = query.offset.unwrap_or(0);
    let limit = std::cmp::min(query.limit.get(), MAX_POOLS_PER_PAGE);
    let fields = create_field_table(query.fields())?;

    crate::establish_connection()?.transaction(|conn| {
        let mut search_criteria = search::pool::parse_search_criteria(query.criteria())?;
        search_criteria.add_offset_and_limit(offset, limit);
        let count_query = search::pool::build_query(&search_criteria)?;
        let sql_query = search::pool::build_query(&search_criteria)?;

        let total = count_query.count().first(conn)?;
        let selected_tags: Vec<i32> = search::pool::get_ordered_ids(conn, sql_query, &search_criteria)?;
        Ok(PagedPoolInfo {
            query: query.query.query,
            offset,
            limit,
            total,
            results: PoolInfo::new_batch_from_ids(conn, selected_tags, &fields)?,
        })
    })
}

fn get_pool(pool_id: i32, auth: AuthResult, query: ResourceQuery) -> ApiResult<PoolInfo> {
    let client = auth?;
    api::verify_privilege(client.as_ref(), config::privileges().pool_view)?;

    let fields = create_field_table(query.fields())?;
    crate::establish_connection()?
        .transaction(|conn| PoolInfo::new_from_id(conn, pool_id, &fields).map_err(api::Error::from))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NewPoolInfo {
    names: Vec<String>,
    category: String,
    description: Option<String>,
    posts: Option<Vec<i32>>,
}

fn create_pool(auth: AuthResult, query: ResourceQuery, pool_info: NewPoolInfo) -> ApiResult<PoolInfo> {
    let client = auth?;
    api::verify_privilege(client.as_ref(), config::privileges().pool_create)?;

    if pool_info.names.is_empty() {
        return Err(api::Error::NoNamesGiven);
    }
    pool_info
        .names
        .iter()
        .map(|name| api::verify_matches_regex(name, RegexType::Pool))
        .collect::<Result<_, _>>()?;

    let fields = create_field_table(query.fields())?;
    crate::establish_connection()?.transaction(|conn| {
        let category_id: i32 = pool_category::table
            .select(pool_category::id)
            .filter(pool_category::name.eq(pool_info.category))
            .first(conn)?;
        let new_pool = NewPool {
            category_id,
            description: pool_info.description.unwrap_or_default(),
        };
        let pool = diesel::insert_into(pool::table)
            .values(new_pool)
            .returning(Pool::as_returning())
            .get_result(conn)?;

        update::pool::add_names(conn, pool.id, 0, pool_info.names)?;
        update::pool::add_posts(conn, pool.id, 0, pool_info.posts.unwrap_or_default())?;

        PoolInfo::new(conn, pool, &fields).map_err(api::Error::from)
    })
}

fn merge_pools(auth: AuthResult, query: ResourceQuery, merge_info: MergeRequest<i32>) -> ApiResult<PoolInfo> {
    let client = auth?;
    api::verify_privilege(client.as_ref(), config::privileges().pool_merge)?;

    let remove_id: i32 = merge_info.remove;
    let merge_to_id: i32 = merge_info.merge_to;
    if remove_id == merge_to_id {
        return Err(api::Error::SelfMerge);
    }

    let fields = create_field_table(query.fields())?;
    crate::establish_connection()?.transaction(|conn| {
        let remove_version = pool::table.find(remove_id).select(pool::last_edit_time).first(conn)?;
        let merge_to_version = pool::table.find(merge_to_id).select(pool::last_edit_time).first(conn)?;
        api::verify_version(remove_version, merge_info.remove_version)?;
        api::verify_version(merge_to_version, merge_info.merge_to_version)?;

        // Merge posts
        let removed_pool_posts: Vec<i32> = diesel::delete(pool_post::table)
            .filter(pool_post::pool_id.eq(remove_id))
            .returning(pool_post::post_id)
            .get_results(conn)?;
        let post_count: i64 = pool_post::table
            .filter(pool_post::pool_id.eq(merge_to_id))
            .count()
            .first(conn)?;
        update::pool::add_posts(conn, merge_to_id, post_count as i32, removed_pool_posts)?;

        diesel::delete(pool::table.find(remove_id)).execute(conn)?;
        PoolInfo::new_from_id(conn, merge_to_id, &fields).map_err(api::Error::from)
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PoolUpdateInfo {
    version: DateTime,
    category: Option<String>,
    description: Option<String>,
    names: Option<Vec<String>>,
    posts: Option<Vec<i32>>,
}

fn update_pool(pool_id: i32, auth: AuthResult, query: ResourceQuery, update: PoolUpdateInfo) -> ApiResult<PoolInfo> {
    let client = auth?;
    let fields = create_field_table(query.fields())?;

    crate::establish_connection()?.transaction(|conn| {
        let pool_version: DateTime = pool::table.find(pool_id).select(pool::last_edit_time).first(conn)?;
        api::verify_version(pool_version, update.version)?;

        if let Some(category) = update.category {
            api::verify_privilege(client.as_ref(), config::privileges().pool_edit_category)?;

            let category_id: i32 = pool_category::table
                .select(pool_category::id)
                .filter(pool_category::name.eq(category))
                .first(conn)?;
            diesel::update(pool::table.find(pool_id))
                .set(pool::category_id.eq(category_id))
                .execute(conn)?;
        }

        if let Some(description) = update.description {
            api::verify_privilege(client.as_ref(), config::privileges().pool_edit_description)?;

            diesel::update(pool::table.find(pool_id))
                .set(pool::description.eq(description))
                .execute(conn)?;
        }

        if let Some(names) = update.names {
            api::verify_privilege(client.as_ref(), config::privileges().pool_edit_name)?;
            update::pool::delete_names(conn, pool_id)?;
            update::pool::add_names(conn, pool_id, 0, names)?;
        }

        // TODO: Optimize
        if let Some(posts) = update.posts {
            api::verify_privilege(client.as_ref(), config::privileges().pool_edit_post)?;
            update::pool::delete_posts(conn, pool_id)?;
            update::pool::add_posts(conn, pool_id, 0, posts)?;
        }

        PoolInfo::new_from_id(conn, pool_id, &fields).map_err(api::Error::from)
    })
}

fn delete_pool(name: String, auth: AuthResult, client_version: DeleteRequest) -> ApiResult<()> {
    let client = auth?;
    api::verify_privilege(client.as_ref(), config::privileges().pool_delete)?;

    let name = percent_encoding::percent_decode_str(&name).decode_utf8()?;
    crate::establish_connection()?.transaction(|conn| {
        let (pool_id, pool_version): (i32, DateTime) = pool::table
            .select((pool::id, pool::last_edit_time))
            .inner_join(pool_name::table)
            .filter(pool_name::name.eq(name))
            .first(conn)?;
        api::verify_version(pool_version, *client_version)?;

        diesel::delete(pool::table.find(pool_id)).execute(conn)?;
        Ok(())
    })
}
