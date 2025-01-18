use crate::api::{ApiResult, AuthResult, DeleteRequest, MergeRequest, PagedQuery, PagedResponse, ResourceQuery};
use crate::model::enums::ResourceType;
use crate::model::pool::{NewPool, Pool};
use crate::resource::pool::{FieldTable, PoolInfo};
use crate::schema::{database_statistics, pool, pool_category, pool_name, pool_post};
use crate::time::DateTime;
use crate::{api, config, db, resource, search, update};
use diesel::dsl::{exists, max};
use diesel::prelude::*;
use serde::Deserialize;
use warp::{Filter, Rejection, Reply};

pub fn routes() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    let list_pools = warp::get()
        .and(api::auth())
        .and(warp::path!("pools"))
        .and(warp::query())
        .map(list_pools)
        .map(api::Reply::from);
    let get_pool = warp::get()
        .and(api::auth())
        .and(warp::path!("pool" / i32))
        .and(api::resource_query())
        .map(get_pool)
        .map(api::Reply::from);
    let create_pool = warp::post()
        .and(api::auth())
        .and(warp::path!("pool"))
        .and(api::resource_query())
        .and(warp::body::json())
        .map(create_pool)
        .map(api::Reply::from);
    let merge_pools = warp::post()
        .and(api::auth())
        .and(warp::path!("pool-merge"))
        .and(api::resource_query())
        .and(warp::body::json())
        .map(merge_pools)
        .map(api::Reply::from);
    let update_pool = warp::put()
        .and(api::auth())
        .and(warp::path!("pool" / i32))
        .and(api::resource_query())
        .and(warp::body::json())
        .map(update_pool)
        .map(api::Reply::from);
    let delete_pool = warp::delete()
        .and(api::auth())
        .and(warp::path!("pool" / String))
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

const MAX_POOLS_PER_PAGE: i64 = 50;

fn create_field_table(fields: Option<&str>) -> Result<FieldTable<bool>, Box<dyn std::error::Error>> {
    fields
        .map(resource::pool::Field::create_table)
        .transpose()
        .map(|opt_table| opt_table.unwrap_or(FieldTable::filled(true)))
        .map_err(Box::from)
}

fn list_pools(auth: AuthResult, query: PagedQuery) -> ApiResult<PagedResponse<PoolInfo>> {
    let client = auth?;
    query.bump_login(client)?;
    api::verify_privilege(client, config::privileges().pool_list)?;

    let offset = query.offset.unwrap_or(0);
    let limit = std::cmp::min(query.limit.get(), MAX_POOLS_PER_PAGE);
    let fields = create_field_table(query.fields())?;

    db::get_connection()?.transaction(|conn| {
        let mut search_criteria = search::pool::parse_search_criteria(query.criteria())?;
        search_criteria.add_offset_and_limit(offset, limit);
        let sql_query = search::pool::build_query(&search_criteria)?;

        let total = if search_criteria.has_filter() {
            let count_query = search::pool::build_query(&search_criteria)?;
            count_query.count().first(conn)?
        } else {
            let pool_count: i32 = database_statistics::table
                .select(database_statistics::pool_count)
                .first(conn)?;
            i64::from(pool_count)
        };

        let selected_tags: Vec<i32> = search::pool::get_ordered_ids(conn, sql_query, &search_criteria)?;
        Ok(PagedResponse {
            query: query.query.query,
            offset,
            limit,
            total,
            results: PoolInfo::new_batch_from_ids(conn, selected_tags, &fields)?,
        })
    })
}

fn get_pool(auth: AuthResult, pool_id: i32, query: ResourceQuery) -> ApiResult<PoolInfo> {
    let client = auth?;
    query.bump_login(client)?;
    api::verify_privilege(client, config::privileges().pool_view)?;

    let fields = create_field_table(query.fields())?;
    db::get_connection()?.transaction(|conn| {
        let pool_exists: bool = diesel::select(exists(pool::table.find(pool_id))).get_result(conn)?;
        if !pool_exists {
            return Err(api::Error::NotFound(ResourceType::Pool));
        }
        PoolInfo::new_from_id(conn, pool_id, &fields).map_err(api::Error::from)
    })
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
    query.bump_login(client)?;
    api::verify_privilege(client, config::privileges().pool_create)?;

    if pool_info.names.is_empty() {
        return Err(api::Error::NoNamesGiven(ResourceType::Pool));
    }

    let fields = create_field_table(query.fields())?;
    let mut conn = db::get_connection()?;
    let pool = conn.transaction(|conn| {
        let category_id: i32 = pool_category::table
            .select(pool_category::id)
            .filter(pool_category::name.eq(pool_info.category))
            .first(conn)?;
        let new_pool = NewPool {
            category_id,
            description: pool_info.description.as_deref().unwrap_or(""),
        };
        let pool = diesel::insert_into(pool::table)
            .values(new_pool)
            .returning(Pool::as_returning())
            .get_result(conn)?;

        update::pool::add_names(conn, pool.id, 0, pool_info.names)?;
        update::pool::add_posts(conn, pool.id, 0, pool_info.posts.unwrap_or_default())?;
        Ok::<_, api::Error>(pool)
    })?;
    conn.transaction(|conn| PoolInfo::new(conn, pool, &fields).map_err(api::Error::from))
}

fn merge_pools(auth: AuthResult, query: ResourceQuery, merge_info: MergeRequest<i32>) -> ApiResult<PoolInfo> {
    let client = auth?;
    query.bump_login(client)?;
    api::verify_privilege(client, config::privileges().pool_merge)?;

    let remove_id = merge_info.remove;
    let merge_to_id = merge_info.merge_to;
    if remove_id == merge_to_id {
        return Err(api::Error::SelfMerge(ResourceType::Pool));
    }

    let fields = create_field_table(query.fields())?;
    let mut conn = db::get_connection()?;
    conn.transaction(|conn| {
        let remove_version = pool::table.find(remove_id).select(pool::last_edit_time).first(conn)?;
        let merge_to_version = pool::table.find(merge_to_id).select(pool::last_edit_time).first(conn)?;
        api::verify_version(remove_version, merge_info.remove_version)?;
        api::verify_version(merge_to_version, merge_info.merge_to_version)?;

        // Merge posts
        let merge_to_pool_posts = pool_post::table
            .select(pool_post::post_id)
            .filter(pool_post::pool_id.eq(merge_to_id))
            .into_boxed();
        let new_pool_posts: Vec<_> = pool_post::table
            .select(pool_post::post_id)
            .filter(pool_post::pool_id.eq(remove_id))
            .filter(pool_post::post_id.ne_all(merge_to_pool_posts))
            .order_by(pool_post::order)
            .load(conn)?;
        let post_count: i64 = pool_post::table
            .filter(pool_post::pool_id.eq(merge_to_id))
            .count()
            .first(conn)?;
        update::pool::add_posts(conn, merge_to_id, post_count as i32, new_pool_posts)?;

        // Merge names
        let current_name_count = pool_name::table
            .select(max(pool_name::order) + 1)
            .filter(pool_name::pool_id.eq(merge_to_id))
            .first::<Option<_>>(conn)?
            .unwrap_or(0);
        let removed_names = diesel::delete(pool_name::table.filter(pool_name::pool_id.eq(remove_id)))
            .returning(pool_name::name)
            .get_results(conn)?;
        update::pool::add_names(conn, merge_to_id, current_name_count, removed_names)?;

        // Update last_edit_time
        diesel::update(pool::table)
            .set(pool::last_edit_time.eq(DateTime::now()))
            .filter(pool::id.eq(merge_to_id))
            .execute(conn)?;

        diesel::delete(pool::table.find(remove_id))
            .execute(conn)
            .map_err(api::Error::from)
    })?;
    conn.transaction(|conn| PoolInfo::new_from_id(conn, merge_to_id, &fields).map_err(api::Error::from))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PoolUpdate {
    version: DateTime,
    category: Option<String>,
    description: Option<String>,
    names: Option<Vec<String>>,
    posts: Option<Vec<i32>>,
}

fn update_pool(auth: AuthResult, pool_id: i32, query: ResourceQuery, update: PoolUpdate) -> ApiResult<PoolInfo> {
    let client = auth?;
    query.bump_login(client)?;
    let fields = create_field_table(query.fields())?;

    let mut conn = db::get_connection()?;
    conn.transaction(|conn| {
        let pool_version: DateTime = pool::table.find(pool_id).select(pool::last_edit_time).first(conn)?;
        api::verify_version(pool_version, update.version)?;

        if let Some(category) = update.category {
            api::verify_privilege(client, config::privileges().pool_edit_category)?;

            let category_id: i32 = pool_category::table
                .select(pool_category::id)
                .filter(pool_category::name.eq(category))
                .first(conn)?;
            diesel::update(pool::table.find(pool_id))
                .set(pool::category_id.eq(category_id))
                .execute(conn)?;
        }
        if let Some(description) = update.description {
            api::verify_privilege(client, config::privileges().pool_edit_description)?;

            diesel::update(pool::table.find(pool_id))
                .set(pool::description.eq(description))
                .execute(conn)?;
        }
        if let Some(names) = update.names {
            api::verify_privilege(client, config::privileges().pool_edit_name)?;
            if names.is_empty() {
                return Err(api::Error::NoNamesGiven(ResourceType::Pool));
            }

            update::pool::delete_names(conn, pool_id)?;
            update::pool::add_names(conn, pool_id, 0, names)?;
        }
        if let Some(posts) = update.posts {
            api::verify_privilege(client, config::privileges().pool_edit_post)?;
            update::pool::delete_posts(conn, pool_id)?;
            update::pool::add_posts(conn, pool_id, 0, posts)?;
        }

        // Update last_edit_time
        diesel::update(pool::table)
            .set(pool::last_edit_time.eq(DateTime::now()))
            .filter(pool::id.eq(pool_id))
            .execute(conn)
            .map_err(api::Error::from)
    })?;
    conn.transaction(|conn| PoolInfo::new_from_id(conn, pool_id, &fields).map_err(api::Error::from))
}

fn delete_pool(auth: AuthResult, name: String, client_version: DeleteRequest) -> ApiResult<()> {
    let client = auth?;
    api::verify_privilege(client, config::privileges().pool_delete)?;

    let name = percent_encoding::percent_decode_str(&name).decode_utf8()?;
    db::get_connection()?.transaction(|conn| {
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

#[cfg(test)]
mod test {
    use crate::api::ApiResult;
    use crate::model::pool::Pool;
    use crate::schema::{database_statistics, pool, pool_name, pool_statistics};
    use crate::test::*;
    use crate::time::DateTime;
    use diesel::dsl::exists;
    use diesel::prelude::*;
    use serial_test::{parallel, serial};

    // Exclude fields that involve creation_time or last_edit_time
    const FIELDS: &str = "&fields=id,description,category,names,posts,postCount";

    #[tokio::test]
    #[parallel]
    async fn list() -> ApiResult<()> {
        const QUERY: &str = "GET /pools/?query";
        const SORT: &str = "-sort:creation-time&limit=40";
        verify_query(&format!("{QUERY}={SORT}{FIELDS}"), "pool/list.json").await?;
        verify_query(&format!("{QUERY}=sort:post-count&limit=1{FIELDS}"), "pool/list_most_posts.json").await?;
        verify_query(&format!("{QUERY}=category:Setting {SORT}{FIELDS}"), "pool/list_category_setting.json").await?;
        verify_query(&format!("{QUERY}=name:*punk* {SORT}{FIELDS}"), "pool/list_name_punk.json").await
    }

    #[tokio::test]
    #[parallel]
    async fn get() -> ApiResult<()> {
        const POOL_ID: i32 = 4;
        let get_last_edit_time = |conn: &mut PgConnection| -> QueryResult<DateTime> {
            pool::table
                .select(pool::last_edit_time)
                .filter(pool::id.eq(POOL_ID))
                .first(conn)
        };

        let mut conn = get_connection()?;
        let last_edit_time = get_last_edit_time(&mut conn)?;

        verify_query(&format!("GET /pool/{POOL_ID}/?{FIELDS}"), "pool/get.json").await?;

        let new_last_edit_time = get_last_edit_time(&mut conn)?;
        assert_eq!(new_last_edit_time, last_edit_time);
        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn create() -> ApiResult<()> {
        let get_pool_count = |conn: &mut PgConnection| -> QueryResult<i32> {
            database_statistics::table
                .select(database_statistics::pool_count)
                .first(conn)
        };

        let mut conn = get_connection()?;
        let pool_count = get_pool_count(&mut conn)?;

        verify_query(&format!("POST /pool/?{FIELDS}"), "pool/create.json").await?;

        let (pool_id, name): (i32, String) = pool::table
            .inner_join(pool_name::table)
            .select((pool::id, pool_name::name))
            .order_by(pool::id.desc())
            .first(&mut conn)?;

        let new_pool_count = get_pool_count(&mut conn)?;
        let post_count: i32 = pool_statistics::table
            .select(pool_statistics::post_count)
            .filter(pool_statistics::pool_id.eq(pool_id))
            .first(&mut conn)?;
        assert_eq!(new_pool_count, pool_count + 1);
        assert_eq!(post_count, 2);

        verify_query(&format!("DELETE /pool/{name}/?{FIELDS}"), "delete.json").await?;

        let new_pool_count = get_pool_count(&mut conn)?;
        let has_pool: bool = diesel::select(exists(pool::table.find(pool_id))).get_result(&mut conn)?;
        assert_eq!(new_pool_count, pool_count);
        assert!(!has_pool);
        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn merge() -> ApiResult<()> {
        const REMOVE_ID: i32 = 2;
        const MERGE_TO_ID: i32 = 5;
        let get_pool_info = |conn: &mut PgConnection| -> QueryResult<(Pool, i32)> {
            pool::table
                .inner_join(pool_statistics::table)
                .select((Pool::as_select(), pool_statistics::post_count))
                .filter(pool::id.eq(MERGE_TO_ID))
                .first(conn)
        };

        let mut conn = get_connection()?;
        let (pool, post_count) = get_pool_info(&mut conn)?;

        verify_query(&format!("POST /pool-merge/?{FIELDS}"), "pool/merge.json").await?;

        let has_pool: bool = diesel::select(exists(pool::table.find(REMOVE_ID))).get_result(&mut conn)?;
        assert!(!has_pool);

        let (new_pool, new_post_count) = get_pool_info(&mut conn)?;
        assert_eq!(new_pool.category_id, pool.category_id);
        assert_eq!(new_pool.description, pool.description);
        assert_eq!(new_pool.creation_time, pool.creation_time);
        assert!(new_pool.last_edit_time > pool.last_edit_time);
        assert_ne!(new_post_count, post_count);
        Ok(reset_database())
    }

    #[tokio::test]
    #[serial]
    async fn update() -> ApiResult<()> {
        const POOL_ID: i32 = 2;
        let get_pool_info = |conn: &mut PgConnection| -> QueryResult<(Pool, i32)> {
            pool::table
                .inner_join(pool_statistics::table)
                .select((Pool::as_select(), pool_statistics::post_count))
                .filter(pool::id.eq(POOL_ID))
                .first(conn)
        };

        let mut conn = get_connection()?;
        let (pool, post_count) = get_pool_info(&mut conn)?;

        verify_query(&format!("PUT /pool/{POOL_ID}/?{FIELDS}"), "pool/update.json").await?;

        let (new_pool, new_post_count) = get_pool_info(&mut conn)?;
        assert_ne!(new_pool.category_id, pool.category_id);
        assert_ne!(new_pool.description, pool.description);
        assert_eq!(new_pool.creation_time, pool.creation_time);
        assert!(new_pool.last_edit_time > pool.last_edit_time);
        assert_ne!(new_post_count, post_count);

        verify_query(&format!("PUT /pool/{POOL_ID}/?{FIELDS}"), "pool/update_restore.json").await?;

        let (new_pool, new_post_count) = get_pool_info(&mut conn)?;
        assert_eq!(new_pool.category_id, pool.category_id);
        assert_eq!(new_pool.description, pool.description);
        assert_eq!(new_pool.creation_time, pool.creation_time);
        assert!(new_pool.last_edit_time > pool.last_edit_time);
        assert_eq!(new_post_count, post_count);
        Ok(())
    }
}
