use crate::api::{ApiResult, AuthResult, DeleteBody, MergeBody, PageParams, PagedResponse, ResourceParams};
use crate::model::enums::ResourceType;
use crate::model::pool::{NewPool, Pool};
use crate::resource::pool::PoolInfo;
use crate::schema::{database_statistics, pool, pool_category, pool_name, pool_post};
use crate::string::SmallString;
use crate::time::DateTime;
use crate::{api, config, db, resource, search, update};
use diesel::dsl::{exists, max};
use diesel::prelude::*;
use serde::Deserialize;
use warp::{Filter, Rejection, Reply};

pub fn routes() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    let list = warp::get()
        .and(api::auth())
        .and(warp::path!("pools"))
        .and(warp::query())
        .map(list)
        .map(api::Reply::from);
    let get = warp::get()
        .and(api::auth())
        .and(warp::path!("pool" / i64))
        .and(api::resource_query())
        .map(get)
        .map(api::Reply::from);
    let create = warp::post()
        .and(api::auth())
        .and(warp::path!("pool"))
        .and(api::resource_query())
        .and(warp::body::json())
        .map(create)
        .map(api::Reply::from);
    let merge = warp::post()
        .and(api::auth())
        .and(warp::path!("pool-merge"))
        .and(api::resource_query())
        .and(warp::body::json())
        .map(merge)
        .map(api::Reply::from);
    let update = warp::put()
        .and(api::auth())
        .and(warp::path!("pool" / i64))
        .and(api::resource_query())
        .and(warp::body::json())
        .map(update)
        .map(api::Reply::from);
    let delete = warp::delete()
        .and(api::auth())
        .and(warp::path!("pool" / String))
        .and(warp::body::json())
        .map(delete)
        .map(api::Reply::from);

    list.or(get).or(create).or(merge).or(update).or(delete)
}

const MAX_POOLS_PER_PAGE: i64 = 1000;

fn list(auth: AuthResult, params: PageParams) -> ApiResult<PagedResponse<PoolInfo>> {
    let client = auth?;
    params.bump_login(client)?;
    api::verify_privilege(client, config::privileges().pool_list)?;

    let offset = params.offset.unwrap_or(0);
    let limit = std::cmp::min(params.limit.get(), MAX_POOLS_PER_PAGE);
    let fields = resource::create_table(params.fields()).map_err(Box::from)?;

    db::get_connection()?.transaction(|conn| {
        let mut search_criteria = search::pool::parse_search_criteria(params.criteria())?;
        search_criteria.add_offset_and_limit(offset, limit);
        let sql_query = search::pool::build_query(&search_criteria)?;

        let total = if search_criteria.has_filter() {
            let count_query = search::pool::build_query(&search_criteria)?;
            count_query.count().first(conn)?
        } else {
            database_statistics::table
                .select(database_statistics::pool_count)
                .first(conn)?
        };

        let selected_tags = search::pool::get_ordered_ids(conn, sql_query, &search_criteria)?;
        Ok(PagedResponse {
            query: params.into_query(),
            offset,
            limit,
            total,
            results: PoolInfo::new_batch_from_ids(conn, selected_tags, &fields)?,
        })
    })
}

fn get(auth: AuthResult, pool_id: i64, params: ResourceParams) -> ApiResult<PoolInfo> {
    let client = auth?;
    params.bump_login(client)?;
    api::verify_privilege(client, config::privileges().pool_view)?;

    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
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
struct CreateBody {
    names: Vec<SmallString>,
    category: SmallString,
    description: Option<String>,
    posts: Option<Vec<i64>>,
}

fn create(auth: AuthResult, params: ResourceParams, body: CreateBody) -> ApiResult<PoolInfo> {
    let client = auth?;
    params.bump_login(client)?;
    api::verify_privilege(client, config::privileges().pool_create)?;

    if body.names.is_empty() {
        return Err(api::Error::NoNamesGiven(ResourceType::Pool));
    }

    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    let mut conn = db::get_connection()?;
    let pool = conn.transaction(|conn| {
        let category_id: i64 = pool_category::table
            .select(pool_category::id)
            .filter(pool_category::name.eq(body.category))
            .first(conn)?;
        let new_pool = NewPool {
            category_id,
            description: body.description.as_deref().unwrap_or(""),
        };
        let pool = diesel::insert_into(pool::table)
            .values(new_pool)
            .returning(Pool::as_returning())
            .get_result(conn)?;

        update::pool::add_names(conn, pool.id, 0, body.names)?;
        update::pool::add_posts(conn, pool.id, 0, body.posts.unwrap_or_default())?;
        Ok::<_, api::Error>(pool)
    })?;
    conn.transaction(|conn| PoolInfo::new(conn, pool, &fields).map_err(api::Error::from))
}

fn merge(auth: AuthResult, params: ResourceParams, body: MergeBody<i64>) -> ApiResult<PoolInfo> {
    let client = auth?;
    params.bump_login(client)?;
    api::verify_privilege(client, config::privileges().pool_merge)?;

    let remove_id = body.remove;
    let merge_to_id = body.merge_to;
    if remove_id == merge_to_id {
        return Err(api::Error::SelfMerge(ResourceType::Pool));
    }

    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    let mut conn = db::get_connection()?;
    conn.transaction(|conn| {
        let remove_version = pool::table.find(remove_id).select(pool::last_edit_time).first(conn)?;
        let merge_to_version = pool::table.find(merge_to_id).select(pool::last_edit_time).first(conn)?;
        api::verify_version(remove_version, body.remove_version)?;
        api::verify_version(merge_to_version, body.merge_to_version)?;

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
        update::pool::add_posts(conn, merge_to_id, post_count, new_pool_posts)?;

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

        diesel::delete(pool::table.find(remove_id)).execute(conn)?;
        update::pool::last_edit_time(conn, merge_to_id)
    })?;
    conn.transaction(|conn| PoolInfo::new_from_id(conn, merge_to_id, &fields).map_err(api::Error::from))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdateBody {
    version: DateTime,
    category: Option<SmallString>,
    description: Option<String>,
    names: Option<Vec<SmallString>>,
    posts: Option<Vec<i64>>,
}

fn update(auth: AuthResult, pool_id: i64, params: ResourceParams, body: UpdateBody) -> ApiResult<PoolInfo> {
    let client = auth?;
    params.bump_login(client)?;
    let fields = resource::create_table(params.fields()).map_err(Box::from)?;

    let mut conn = db::get_connection()?;
    conn.transaction(|conn| {
        let pool_version: DateTime = pool::table.find(pool_id).select(pool::last_edit_time).first(conn)?;
        api::verify_version(pool_version, body.version)?;

        if let Some(category) = body.category {
            api::verify_privilege(client, config::privileges().pool_edit_category)?;

            let category_id: i64 = pool_category::table
                .select(pool_category::id)
                .filter(pool_category::name.eq(category))
                .first(conn)?;
            diesel::update(pool::table.find(pool_id))
                .set(pool::category_id.eq(category_id))
                .execute(conn)?;
        }
        if let Some(description) = body.description {
            api::verify_privilege(client, config::privileges().pool_edit_description)?;

            diesel::update(pool::table.find(pool_id))
                .set(pool::description.eq(description))
                .execute(conn)?;
        }
        if let Some(names) = body.names {
            api::verify_privilege(client, config::privileges().pool_edit_name)?;
            if names.is_empty() {
                return Err(api::Error::NoNamesGiven(ResourceType::Pool));
            }

            update::pool::delete_names(conn, pool_id)?;
            update::pool::add_names(conn, pool_id, 0, names)?;
        }
        if let Some(posts) = body.posts {
            api::verify_privilege(client, config::privileges().pool_edit_post)?;
            update::pool::delete_posts(conn, pool_id)?;
            update::pool::add_posts(conn, pool_id, 0, posts)?;
        }
        update::pool::last_edit_time(conn, pool_id)
    })?;
    conn.transaction(|conn| PoolInfo::new_from_id(conn, pool_id, &fields).map_err(api::Error::from))
}

fn delete(auth: AuthResult, name: String, client_version: DeleteBody) -> ApiResult<()> {
    let client = auth?;
    api::verify_privilege(client, config::privileges().pool_delete)?;

    let name = percent_encoding::percent_decode_str(&name).decode_utf8()?;
    db::get_connection()?.transaction(|conn| {
        let (pool_id, pool_version): (i64, DateTime) = pool::table
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
        const POOL_ID: i64 = 4;
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
        let get_pool_count = |conn: &mut PgConnection| -> QueryResult<i64> {
            database_statistics::table
                .select(database_statistics::pool_count)
                .first(conn)
        };

        let mut conn = get_connection()?;
        let pool_count = get_pool_count(&mut conn)?;

        verify_query(&format!("POST /pool/?{FIELDS}"), "pool/create.json").await?;

        let (pool_id, name): (i64, String) = pool_name::table
            .select((pool_name::pool_id, pool_name::name))
            .order_by(pool_name::pool_id.desc())
            .first(&mut conn)?;

        let new_pool_count = get_pool_count(&mut conn)?;
        let post_count: i64 = pool_statistics::table
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
        const REMOVE_ID: i64 = 2;
        const MERGE_TO_ID: i64 = 5;
        let get_pool_info = |conn: &mut PgConnection| -> QueryResult<(Pool, i64)> {
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
        const POOL_ID: i64 = 2;
        let get_pool_info = |conn: &mut PgConnection| -> QueryResult<(Pool, i64)> {
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
