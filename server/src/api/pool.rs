use crate::api::{ApiResult, DeleteBody, MergeBody, PageParams, PagedResponse, ResourceParams};
use crate::auth::Client;
use crate::model::enums::ResourceType;
use crate::model::pool::{NewPool, Pool};
use crate::resource::pool::PoolInfo;
use crate::schema::{pool, pool_category};
use crate::search::pool::QueryBuilder;
use crate::snapshot::pool::SnapshotData;
use crate::string::{LargeString, SmallString};
use crate::time::DateTime;
use crate::{api, config, db, resource, snapshot, update};
use axum::extract::{Extension, Path, Query};
use axum::{Json, Router, routing};
use diesel::dsl::exists;
use diesel::prelude::*;
use serde::Deserialize;

pub fn routes() -> Router {
    Router::new()
        .route("/pools", routing::get(list))
        .route("/pool", routing::post(create))
        .route("/pool/{id}", routing::get(get).put(update).delete(delete))
        .route("/pool-merge", routing::post(merge))
}

const MAX_POOLS_PER_PAGE: i64 = 1000;

async fn list(
    Extension(client): Extension<Client>,
    Query(params): Query<PageParams>,
) -> ApiResult<Json<PagedResponse<PoolInfo>>> {
    api::verify_privilege(client, config::privileges().pool_list)?;

    let offset = params.offset.unwrap_or(0);
    let limit = std::cmp::min(params.limit.get(), MAX_POOLS_PER_PAGE);
    let fields = resource::create_table(params.fields()).map_err(Box::from)?;

    db::get_connection()?.transaction(|conn| {
        let mut query_builder = QueryBuilder::new(params.criteria())?;
        query_builder.set_offset_and_limit(offset, limit);

        let total = query_builder.count(conn)?;
        let selected_pools = query_builder.load(conn)?;
        Ok(Json(PagedResponse {
            query: params.into_query(),
            offset,
            limit,
            total,
            results: PoolInfo::new_batch_from_ids(conn, &selected_pools, &fields)?,
        }))
    })
}

async fn get(
    Extension(client): Extension<Client>,
    Path(pool_id): Path<i64>,
    Query(params): Query<ResourceParams>,
) -> ApiResult<Json<PoolInfo>> {
    api::verify_privilege(client, config::privileges().pool_view)?;

    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    db::get_connection()?.transaction(|conn| {
        let pool_exists: bool = diesel::select(exists(pool::table.find(pool_id))).get_result(conn)?;
        if !pool_exists {
            return Err(api::Error::NotFound(ResourceType::Pool));
        }
        PoolInfo::new_from_id(conn, pool_id, &fields)
            .map(Json)
            .map_err(api::Error::from)
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateBody {
    names: Vec<SmallString>,
    category: SmallString,
    description: Option<LargeString>,
    posts: Option<Vec<i64>>,
}

async fn create(
    Extension(client): Extension<Client>,
    Query(params): Query<ResourceParams>,
    Json(body): Json<CreateBody>,
) -> ApiResult<Json<PoolInfo>> {
    api::verify_privilege(client, config::privileges().pool_create)?;

    if body.names.is_empty() {
        return Err(api::Error::NoNamesGiven(ResourceType::Pool));
    }

    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    let mut conn = db::get_connection()?;
    let pool = conn.transaction(|conn| {
        let (category_id, category): (i64, SmallString) = pool_category::table
            .select((pool_category::id, pool_category::name))
            .filter(pool_category::name.eq(body.category))
            .first(conn)?;
        let pool: Pool = NewPool {
            category_id,
            description: body.description.as_deref().unwrap_or(""),
        }
        .insert_into(pool::table)
        .get_result(conn)?;

        let posts = body.posts.unwrap_or_default();

        // Add names and posts
        update::pool::add_names(conn, pool.id, 0, &body.names)?;
        update::pool::add_posts(conn, pool.id, 0, &posts)?;

        let pool_data = SnapshotData {
            description: body.description.unwrap_or_default(),
            category,
            names: body.names,
            posts,
        };
        snapshot::pool::creation_snapshot(conn, client, pool.id, pool_data)?;
        Ok::<_, api::Error>(pool)
    })?;
    conn.transaction(|conn| PoolInfo::new(conn, pool, &fields))
        .map(Json)
        .map_err(api::Error::from)
}

async fn merge(
    Extension(client): Extension<Client>,
    Query(params): Query<ResourceParams>,
    Json(body): Json<MergeBody<i64>>,
) -> ApiResult<Json<PoolInfo>> {
    api::verify_privilege(client, config::privileges().pool_merge)?;

    let absorbed_id = body.remove;
    let merge_to_id = body.merge_to;
    if absorbed_id == merge_to_id {
        return Err(api::Error::SelfMerge(ResourceType::Pool));
    }

    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    let mut conn = db::get_connection()?;
    conn.transaction(|conn| {
        let remove_version = pool::table.find(absorbed_id).select(pool::last_edit_time).first(conn)?;
        let merge_to_version = pool::table.find(merge_to_id).select(pool::last_edit_time).first(conn)?;
        api::verify_version(remove_version, body.remove_version)?;
        api::verify_version(merge_to_version, body.merge_to_version)?;

        update::pool::merge(conn, absorbed_id, merge_to_id)?;
        snapshot::pool::merge_snapshot(conn, client, absorbed_id, merge_to_id).map_err(api::Error::from)
    })?;
    conn.transaction(|conn| PoolInfo::new_from_id(conn, merge_to_id, &fields))
        .map(Json)
        .map_err(api::Error::from)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdateBody {
    version: DateTime,
    category: Option<SmallString>,
    description: Option<LargeString>,
    names: Option<Vec<SmallString>>,
    posts: Option<Vec<i64>>,
}

async fn update(
    Extension(client): Extension<Client>,
    Path(pool_id): Path<i64>,
    Query(params): Query<ResourceParams>,
    Json(body): Json<UpdateBody>,
) -> ApiResult<Json<PoolInfo>> {
    let fields = resource::create_table(params.fields()).map_err(Box::from)?;

    let mut conn = db::get_connection()?;
    conn.transaction(|conn| {
        let old_pool: Pool = pool::table.find(pool_id).first(conn)?;
        api::verify_version(old_pool.last_edit_time, body.version)?;

        let mut new_pool = old_pool.clone();
        let old_snapshot_data = SnapshotData::retrieve(conn, old_pool)?;
        let mut new_snapshot_data = old_snapshot_data.clone();

        if let Some(category) = body.category {
            api::verify_privilege(client, config::privileges().pool_edit_category)?;

            let category_id: i64 = pool_category::table
                .select(pool_category::id)
                .filter(pool_category::name.eq(&category))
                .first(conn)?;
            new_pool.category_id = category_id;
            new_snapshot_data.category = category;
        }
        if let Some(description) = body.description {
            api::verify_privilege(client, config::privileges().pool_edit_description)?;
            new_pool.description = description.clone();
            new_snapshot_data.description = description;
        }
        if let Some(names) = body.names {
            api::verify_privilege(client, config::privileges().pool_edit_name)?;
            if names.is_empty() {
                return Err(api::Error::NoNamesGiven(ResourceType::Pool));
            }

            update::pool::delete_names(conn, pool_id)?;
            update::pool::add_names(conn, pool_id, 0, &names)?;
            new_snapshot_data.names = names;
        }
        if let Some(posts) = body.posts {
            api::verify_privilege(client, config::privileges().pool_edit_post)?;

            update::pool::delete_posts(conn, pool_id)?;
            update::pool::add_posts(conn, pool_id, 0, &posts)?;
            new_snapshot_data.posts = posts;
        }

        new_pool.last_edit_time = DateTime::now();
        let _: Pool = new_pool.save_changes(conn)?;
        snapshot::pool::modification_snapshot(conn, client, pool_id, old_snapshot_data, new_snapshot_data)?;
        Ok(())
    })?;
    conn.transaction(|conn| PoolInfo::new_from_id(conn, pool_id, &fields))
        .map(Json)
        .map_err(api::Error::from)
}

async fn delete(
    Extension(client): Extension<Client>,
    Path(pool_id): Path<i64>,
    Json(client_version): Json<DeleteBody>,
) -> ApiResult<Json<()>> {
    api::verify_privilege(client, config::privileges().pool_delete)?;

    db::get_connection()?.transaction(|conn| {
        let pool: Pool = pool::table.find(pool_id).first(conn)?;
        api::verify_version(pool.last_edit_time, *client_version)?;

        let pool_id = pool.id;
        let pool_data = SnapshotData::retrieve(conn, pool)?;
        snapshot::pool::deletion_snapshot(conn, client, pool_id, pool_data)?;

        diesel::delete(pool::table.find(pool_id)).execute(conn)?;
        Ok(Json(()))
    })
}

#[cfg(test)]
mod test {
    use crate::api::ApiResult;
    use crate::model::pool::Pool;
    use crate::schema::{database_statistics, pool, pool_statistics};
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

        let pool_id: i64 = pool::table
            .select(pool::id)
            .order_by(pool::id.desc())
            .first(&mut conn)?;

        let new_pool_count = get_pool_count(&mut conn)?;
        let post_count: i64 = pool_statistics::table
            .select(pool_statistics::post_count)
            .filter(pool_statistics::pool_id.eq(pool_id))
            .first(&mut conn)?;
        assert_eq!(new_pool_count, pool_count + 1);
        assert_eq!(post_count, 2);

        verify_query(&format!("DELETE /pool/{pool_id}/?{FIELDS}"), "pool/delete.json").await?;

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
