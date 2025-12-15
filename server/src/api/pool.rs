use crate::api::error::{ApiError, ApiResult};
use crate::api::extract::{Json, Path, Query};
use crate::api::{DeleteBody, MergeBody, PageParams, PagedResponse, ResourceParams};
use crate::app::AppState;
use crate::auth::Client;
use crate::model::enums::ResourceType;
use crate::model::pool::{NewPool, Pool};
use crate::resource::pool::PoolInfo;
use crate::schema::{pool, pool_category};
use crate::search::Builder;
use crate::search::pool::QueryBuilder;
use crate::snapshot::pool::SnapshotData;
use crate::string::{LargeString, SmallString};
use crate::time::DateTime;
use crate::{api, resource, snapshot, update};
use axum::extract::{Extension, State};
use axum::{Router, routing};
use diesel::dsl::exists;
use diesel::{
    Connection, ExpressionMethods, Insertable, OptionalExtension, PgConnection, QueryDsl, RunQueryDsl, SaveChangesDsl,
};
use serde::Deserialize;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/pools", routing::get(list))
        .route("/pool", routing::post(create))
        .route("/pool/{id}", routing::get(get).put(update).delete(delete))
        .route("/pool-merge", routing::post(merge))
}

const MAX_POOLS_PER_PAGE: i64 = 1000;

/// See [lsting-pools](https://github.com/liamw1/oxibooru/blob/master/doc/API.md#listing-pools)
async fn list(
    State(state): State<AppState>,
    Extension(client): Extension<Client>,
    Query(params): Query<PageParams>,
) -> ApiResult<Json<PagedResponse<PoolInfo>>> {
    api::verify_privilege(client, state.config.privileges().pool_list)?;

    let offset = params.offset.unwrap_or(0);
    let limit = std::cmp::min(params.limit.get(), MAX_POOLS_PER_PAGE);
    let fields = resource::create_table(params.fields()).map_err(Box::from)?;

    state.get_connection()?.transaction(|conn| {
        let mut query_builder = QueryBuilder::new(client, params.criteria())?;
        query_builder.set_offset_and_limit(offset, limit);

        let (total, selected_pools) = query_builder.list(conn)?;
        Ok(Json(PagedResponse {
            query: params.into_query(),
            offset,
            limit,
            total,
            results: PoolInfo::new_batch_from_ids(conn, &state.config, client, &selected_pools, &fields)?,
        }))
    })
}

/// See [getting-pool](https://github.com/liamw1/oxibooru/blob/master/doc/API.md#getting-pool)
async fn get(
    State(state): State<AppState>,
    Extension(client): Extension<Client>,
    Path(pool_id): Path<i64>,
    Query(params): Query<ResourceParams>,
) -> ApiResult<Json<PoolInfo>> {
    api::verify_privilege(client, state.config.privileges().pool_view)?;

    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    state.get_connection()?.transaction(|conn| {
        let pool_exists: bool = diesel::select(exists(pool::table.find(pool_id))).first(conn)?;
        if !pool_exists {
            return Err(ApiError::NotFound(ResourceType::Pool));
        }
        PoolInfo::new_from_id(conn, &state.config, client, pool_id, &fields)
            .map(Json)
            .map_err(ApiError::from)
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

/// See [creating-pool](https://github.com/liamw1/oxibooru/blob/master/doc/API.md#creating-pool)
async fn create(
    State(state): State<AppState>,
    Extension(client): Extension<Client>,
    Query(params): Query<ResourceParams>,
    Json(body): Json<CreateBody>,
) -> ApiResult<Json<PoolInfo>> {
    api::verify_privilege(client, state.config.privileges().pool_create)?;

    if body.names.is_empty() {
        return Err(ApiError::NoNamesGiven(ResourceType::Pool));
    }

    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    let mut conn = state.get_connection()?;
    let pool = conn.transaction(|conn| {
        let (category_id, category): (i64, SmallString) = pool_category::table
            .select((pool_category::id, pool_category::name))
            .filter(pool_category::name.eq(body.category))
            .first(conn)
            .optional()?
            .ok_or(ApiError::NotFound(ResourceType::PoolCategory))?;
        let pool: Pool = NewPool {
            category_id,
            description: body.description.as_deref().unwrap_or(""),
        }
        .insert_into(pool::table)
        .get_result(conn)?;

        let posts = body.posts.unwrap_or_default();

        // Set names and posts
        update::pool::set_names(conn, &state.config, pool.id, &body.names)?;
        update::pool::add_posts(conn, pool.id, 0, &posts)?;

        let pool_data = SnapshotData {
            description: body.description.unwrap_or_default(),
            category,
            names: body.names,
            posts,
        };
        snapshot::pool::creation_snapshot(conn, client, pool.id, pool_data)?;
        Ok::<_, ApiError>(pool)
    })?;
    conn.transaction(|conn| PoolInfo::new(conn, &state.config, client, pool, &fields))
        .map(Json)
        .map_err(ApiError::from)
}

/// See [merging-pools](https://github.com/liamw1/oxibooru/blob/master/doc/API.md#merging-pools)
async fn merge(
    State(state): State<AppState>,
    Extension(client): Extension<Client>,
    Query(params): Query<ResourceParams>,
    Json(body): Json<MergeBody<i64>>,
) -> ApiResult<Json<PoolInfo>> {
    api::verify_privilege(client, state.config.privileges().pool_merge)?;

    let absorbed_id = body.remove;
    let merge_to_id = body.merge_to;
    if absorbed_id == merge_to_id {
        return Err(ApiError::SelfMerge(ResourceType::Pool));
    }

    let get_pool_info = |conn: &mut PgConnection, id: i64| {
        pool::table
            .find(id)
            .select(pool::last_edit_time)
            .first(conn)
            .optional()?
            .ok_or(ApiError::NotFound(ResourceType::Pool))
    };

    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    let mut conn = state.get_connection()?;
    conn.transaction(|conn| {
        let remove_version = get_pool_info(conn, absorbed_id)?;
        let merge_to_version = get_pool_info(conn, merge_to_id)?;
        api::verify_version(remove_version, body.remove_version)?;
        api::verify_version(merge_to_version, body.merge_to_version)?;

        update::pool::merge(conn, absorbed_id, merge_to_id)?;
        snapshot::pool::merge_snapshot(conn, client, absorbed_id, merge_to_id).map_err(ApiError::from)
    })?;
    conn.transaction(|conn| PoolInfo::new_from_id(conn, &state.config, client, merge_to_id, &fields))
        .map(Json)
        .map_err(ApiError::from)
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

/// See [updating-pool](https://github.com/liamw1/oxibooru/blob/master/doc/API.md#updating-pool)
async fn update(
    State(state): State<AppState>,
    Extension(client): Extension<Client>,
    Path(pool_id): Path<i64>,
    Query(params): Query<ResourceParams>,
    Json(body): Json<UpdateBody>,
) -> ApiResult<Json<PoolInfo>> {
    let fields = resource::create_table(params.fields()).map_err(Box::from)?;

    let mut conn = state.get_connection()?;
    conn.transaction(|conn| {
        let old_pool: Pool = pool::table
            .find(pool_id)
            .first(conn)
            .optional()?
            .ok_or(ApiError::NotFound(ResourceType::Pool))?;
        api::verify_version(old_pool.last_edit_time, body.version)?;

        let mut new_pool = old_pool.clone();
        let old_snapshot_data = SnapshotData::retrieve(conn, old_pool)?;
        let mut new_snapshot_data = old_snapshot_data.clone();

        if let Some(category) = body.category {
            api::verify_privilege(client, state.config.privileges().pool_edit_category)?;

            let category_id: i64 = pool_category::table
                .select(pool_category::id)
                .filter(pool_category::name.eq(&category))
                .first(conn)
                .optional()?
                .ok_or(ApiError::NotFound(ResourceType::PoolCategory))?;
            new_pool.category_id = category_id;
            new_snapshot_data.category = category;
        }
        if let Some(description) = body.description {
            api::verify_privilege(client, state.config.privileges().pool_edit_description)?;
            new_pool.description = description.clone();
            new_snapshot_data.description = description;
        }
        if let Some(names) = body.names {
            api::verify_privilege(client, state.config.privileges().pool_edit_name)?;
            if names.is_empty() {
                return Err(ApiError::NoNamesGiven(ResourceType::Pool));
            }

            update::pool::set_names(conn, &state.config, pool_id, &names)?;
            new_snapshot_data.names = names;
        }
        if let Some(mut posts) = body.posts {
            api::verify_privilege(client, state.config.privileges().pool_edit_post)?;

            update::pool::set_posts(conn, &state.config, client, pool_id, &mut posts)?;
            new_snapshot_data.posts = posts;
        }

        new_pool.last_edit_time = DateTime::now();
        let _: Pool = new_pool.save_changes(conn)?;
        snapshot::pool::modification_snapshot(conn, client, pool_id, old_snapshot_data, new_snapshot_data)?;
        Ok(())
    })?;
    conn.transaction(|conn| PoolInfo::new_from_id(conn, &state.config, client, pool_id, &fields))
        .map(Json)
        .map_err(ApiError::from)
}

/// See [deleting-pool](https://github.com/liamw1/oxibooru/blob/master/doc/API.md#deleting-pool)
async fn delete(
    State(state): State<AppState>,
    Extension(client): Extension<Client>,
    Path(pool_id): Path<i64>,
    Json(client_version): Json<DeleteBody>,
) -> ApiResult<Json<()>> {
    api::verify_privilege(client, state.config.privileges().pool_delete)?;

    state.get_connection()?.transaction(|conn| {
        let pool: Pool = pool::table
            .find(pool_id)
            .first(conn)
            .optional()?
            .ok_or(ApiError::NotFound(ResourceType::Pool))?;
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
    use crate::api::error::ApiResult;
    use crate::model::enums::{ResourceType, UserRank};
    use crate::model::pool::Pool;
    use crate::schema::{database_statistics, pool, pool_statistics};
    use crate::search::pool::Token;
    use crate::test::*;
    use crate::time::DateTime;
    use diesel::dsl::exists;
    use diesel::{ExpressionMethods, PgConnection, QueryDsl, QueryResult, RunQueryDsl, SelectableHelper};
    use serial_test::{parallel, serial};
    use strum::IntoEnumIterator;

    // Exclude fields that involve creation_time or last_edit_time
    const FIELDS: &str = "&fields=id,description,category,names,posts,postCount";

    #[tokio::test]
    #[parallel]
    async fn list() -> ApiResult<()> {
        const QUERY: &str = "GET /pools/?query";
        const PARAMS: &str = "-sort:creation-time&limit=40&fields=id";
        verify_response(&format!("{QUERY}=-sort:creation-time&limit=40{FIELDS}"), "pool/list").await?;

        let filter_table = crate::search::pool::filter_table();
        for token in Token::iter() {
            let filter = filter_table[token];
            let (sign, filter) = if filter.starts_with('-') {
                filter.split_at(1)
            } else {
                ("", filter)
            };
            let query = format!("{QUERY}={sign}{token}:{filter} {PARAMS}");
            let path = format!("pool/list_{token}_filtered");
            verify_response(&query, &path).await?;

            let query = format!("{QUERY}=sort:{token} {PARAMS}");
            let path = format!("pool/list_{token}_sorted");
            verify_response(&query, &path).await?;
        }
        Ok(())
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

        verify_response(&format!("GET /pool/{POOL_ID}/?{FIELDS}"), "pool/get").await?;

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

        verify_response(&format!("POST /pool/?{FIELDS}"), "pool/create").await?;

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

        verify_response(&format!("DELETE /pool/{pool_id}/?{FIELDS}"), "pool/delete").await?;

        let new_pool_count = get_pool_count(&mut conn)?;
        let has_pool: bool = diesel::select(exists(pool::table.find(pool_id))).first(&mut conn)?;
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

        verify_response(&format!("POST /pool-merge/?{FIELDS}"), "pool/merge").await?;

        let has_pool: bool = diesel::select(exists(pool::table.find(REMOVE_ID))).first(&mut conn)?;
        assert!(!has_pool);

        let (new_pool, new_post_count) = get_pool_info(&mut conn)?;
        assert_eq!(new_pool.category_id, pool.category_id);
        assert_eq!(new_pool.description, pool.description);
        assert_eq!(new_pool.creation_time, pool.creation_time);
        assert!(new_pool.last_edit_time > pool.last_edit_time);
        assert_ne!(new_post_count, post_count);
        reset_database();
        Ok(())
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

        verify_response(&format!("PUT /pool/{POOL_ID}/?{FIELDS}"), "pool/edit").await?;

        let (new_pool, new_post_count) = get_pool_info(&mut conn)?;
        assert_ne!(new_pool.category_id, pool.category_id);
        assert_ne!(new_pool.description, pool.description);
        assert_eq!(new_pool.creation_time, pool.creation_time);
        assert!(new_pool.last_edit_time > pool.last_edit_time);
        assert_ne!(new_post_count, post_count);

        verify_response(&format!("PUT /pool/{POOL_ID}/?{FIELDS}"), "pool/edit_restore").await?;

        let (new_pool, new_post_count) = get_pool_info(&mut conn)?;
        assert_eq!(new_pool.category_id, pool.category_id);
        assert_eq!(new_pool.description, pool.description);
        assert_eq!(new_pool.creation_time, pool.creation_time);
        assert!(new_pool.last_edit_time > pool.last_edit_time);
        assert_eq!(new_post_count, post_count);
        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn preferences() -> ApiResult<()> {
        verify_response_with_user(
            UserRank::Anonymous,
            "GET /pools/?query=-sort:creation-time&limit=40&fields=id,posts,postCount",
            "pool/list_with_preferences",
        )
        .await?;
        verify_response_with_user(
            UserRank::Anonymous,
            "PUT /pool/2/?fields=id,posts,postCount",
            "pool/edit_with_preferences",
        )
        .await?;

        reset_database();
        Ok(())
    }

    #[tokio::test]
    #[parallel]
    async fn error() -> ApiResult<()> {
        verify_response("GET /pool/99", "pool/get_nonexistent").await?;
        verify_response("POST /pool-merge", "pool/merge_to_nonexistent").await?;
        verify_response("POST /pool-merge", "pool/merge_with_nonexistent").await?;
        verify_response("PUT /pool/99", "pool/edit_nonexistent").await?;
        verify_response("DELETE /pool/99", "pool/delete_nonexistent").await?;

        verify_response("POST /pool", "pool/create_nameless").await?;
        verify_response("POST /pool", "pool/create_name_clash").await?;
        verify_response("POST /pool", "pool/create_invalid_name").await?;
        verify_response("POST /pool", "pool/create_invalid_post").await?;
        verify_response("POST /pool", "pool/create_invalid_category").await?;
        verify_response("POST /pool", "pool/create_duplicate_post").await?;
        verify_response("POST /pool-merge", "pool/self-merge").await?;

        verify_response("PUT /pool/1", "pool/edit_nameless").await?;
        verify_response("PUT /pool/1", "pool/edit_name_clash").await?;
        verify_response("PUT /pool/1", "pool/edit_invalid_name").await?;
        verify_response("PUT /pool/1", "pool/edit_invalid_post").await?;
        verify_response("PUT /pool/1", "pool/edit_invalid_category").await?;
        verify_response("PUT /pool/1", "pool/edit_duplicate_post").await?;

        reset_sequence(ResourceType::Pool)?;
        Ok(())
    }
}
