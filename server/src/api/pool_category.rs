use crate::api::doc::POOL_CATEGORY_TAG;
use crate::api::error::{ApiError, ApiResult};
use crate::api::extract::{Json, Path, Query};
use crate::api::{DeleteBody, ResourceParams, UnpagedResponse, error};
use crate::app::AppState;
use crate::auth::Client;
use crate::config::RegexType;
use crate::model::enums::{ResourceProperty, ResourceType};
use crate::model::pool_category::{NewPoolCategory, PoolCategory};
use crate::resource::pool_category::PoolCategoryInfo;
use crate::schema::{pool, pool_category};
use crate::string::SmallString;
use crate::time::DateTime;
use crate::{api, resource, snapshot};
use axum::extract::{Extension, State};
use diesel::{Connection, ExpressionMethods, Insertable, OptionalExtension, QueryDsl, RunQueryDsl, SaveChangesDsl};
use serde::Deserialize;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list, create))
        .routes(routes!(get, update, delete))
        .routes(routes!(set_default))
}

/// Lists all pool categories.
///
/// Doesn't use paging.
#[utoipa::path(
    get, 
    path = "/pool-categories", 
    tag = POOL_CATEGORY_TAG,
    params(ResourceParams),
    responses(
        (status = 200, description = "List of pool categories", body = UnpagedResponse<PoolCategoryInfo>),
        (status = 403, description = "Privileges are too low"),
    ),
)]
async fn list(
    State(state): State<AppState>,
    Extension(client): Extension<Client>,
    Query(params): Query<ResourceParams>,
) -> ApiResult<Json<UnpagedResponse<PoolCategoryInfo>>> {
    api::verify_privilege(client, state.config.privileges().pool_category_list)?;

    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    state
        .get_connection()?
        .transaction(|conn| PoolCategoryInfo::all(conn, &fields))
        .map(|results| UnpagedResponse { results })
        .map(Json)
        .map_err(ApiError::from)
}

/// Retrieves information about an existing pool category.
#[utoipa::path(
    get, 
    path = "/pool-category/{name}", 
    tag = POOL_CATEGORY_TAG,
    params(
        ("name" = String, Path, description = "Pool category name"),
        ResourceParams,
    ),
    responses(
        (status = 200, body = PoolCategoryInfo),
        (status = 403, description = "Privileges are too low"),
        (status = 404, description = "Pool category does not exist"),
    ),
)]
async fn get(
    State(state): State<AppState>,
    Extension(client): Extension<Client>,
    Path(name): Path<SmallString>,
    Query(params): Query<ResourceParams>,
) -> ApiResult<Json<PoolCategoryInfo>> {
    api::verify_privilege(client, state.config.privileges().pool_category_view)?;

    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    state.get_connection()?.transaction(|conn| {
        let category = pool_category::table
            .filter(pool_category::name.eq(name))
            .first(conn)
            .optional()?
            .ok_or(ApiError::NotFound(ResourceType::PoolCategory))?;
        PoolCategoryInfo::new(conn, category, &fields)
            .map(Json)
            .map_err(ApiError::from)
    })
}

/// Request body for creating a pool category.
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
struct PoolCategoryCreateBody {
    /// Category name. Must match `pool_category_name_regex` from server's configuration.
    name: SmallString,
    /// Category color.
    color: SmallString,
}

/// Creates a new pool category using specified parameters.
///
/// Name must match `pool_category_name_regex` from server's configuration.
/// Names are case insensitive.
#[utoipa::path(
    post, 
    path = "/pool-categories", 
    tag = POOL_CATEGORY_TAG,
    params(ResourceParams),
    request_body = PoolCategoryCreateBody,
    responses(
        (status = 200, body = PoolCategoryInfo),
        (status = 400, description = "Name or color is invalid or missing"),
        (status = 403, description = "Privileges are too low"),
        (status = 409, description = "Name is used by an existing pool category"),
    ),
)]
async fn create(
    State(state): State<AppState>,
    Extension(client): Extension<Client>,
    Query(params): Query<ResourceParams>,
    Json(body): Json<PoolCategoryCreateBody>,
) -> ApiResult<Json<PoolCategoryInfo>> {
    api::verify_privilege(client, state.config.privileges().pool_category_create)?;
    api::verify_matches_regex(&state.config, &body.name, RegexType::PoolCategory)?;
    let fields = resource::create_table(params.fields()).map_err(Box::from)?;

    let new_category = NewPoolCategory {
        name: &body.name,
        color: &body.color,
    };

    let mut conn = state.get_connection()?;
    let category = conn.transaction(|conn| {
        let category = new_category
            .insert_into(pool_category::table)
            .on_conflict(pool_category::name)
            .do_nothing()
            .get_result(conn)
            .optional()?
            .ok_or(ApiError::AlreadyExists(ResourceProperty::PoolCategoryName))?;
        snapshot::pool_category::creation_snapshot(conn, client, &category)?;
        Ok::<_, ApiError>(category)
    })?;
    conn.transaction(|conn| PoolCategoryInfo::new(conn, category, &fields))
        .map(Json)
        .map_err(ApiError::from)
}

/// Request body for updating a pool category.
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
struct PoolCategoryUpdateBody {
    /// Resource version. See [versioning](#Versioning).
    version: DateTime,
    /// New category name. Must match `pool_category_name_regex` from server's configuration.
    name: Option<SmallString>,
    /// New category color.
    color: Option<SmallString>,
}

/// Updates an existing pool category using specified parameters.
///
/// Name must match `pool_category_name_regex` from server's configuration.
/// Names are case insensitive. All fields except `version` are optional -
/// update concerns only provided fields.
#[utoipa::path(
    put, 
    path = "/pool-category/{name}", 
    tag = POOL_CATEGORY_TAG,
    params(
        ("name" = String, Path, description = "Pool category name"),
        ResourceParams,
    ),
    request_body = PoolCategoryUpdateBody,
    responses(
        (status = 200, description = "Updated pool category", body = PoolCategoryInfo),
        (status = 400, description = "Name or color is invalid"),
        (status = 403, description = "Privileges are too low"),
        (status = 404, description = "Pool category does not exist"),
        (status = 409, description = "Version is outdated or name is used by an existing pool category"),
    ),
)]
async fn update(
    State(state): State<AppState>,
    Extension(client): Extension<Client>,
    Path(name): Path<SmallString>,
    Query(params): Query<ResourceParams>,
    Json(body): Json<PoolCategoryUpdateBody>,
) -> ApiResult<Json<PoolCategoryInfo>> {
    let fields = resource::create_table(params.fields()).map_err(Box::from)?;

    let mut conn = state.get_connection()?;
    let updated_category = conn.transaction(|conn| {
        let old_category: PoolCategory = pool_category::table
            .filter(pool_category::name.eq(name))
            .first(conn)
            .optional()?
            .ok_or(ApiError::NotFound(ResourceType::PoolCategory))?;
        api::verify_version(old_category.last_edit_time, body.version)?;

        let mut new_category = old_category.clone();
        if let Some(name) = body.name {
            api::verify_privilege(client, state.config.privileges().pool_category_edit_name)?;
            api::verify_matches_regex(&state.config, &name, RegexType::PoolCategory)?;
            new_category.name = name;
        }
        if let Some(color) = body.color {
            api::verify_privilege(client, state.config.privileges().pool_category_edit_color)?;
            new_category.color = color;
        }

        new_category.last_edit_time = DateTime::now();
        let _: PoolCategory =
            error::map_unique_violation(new_category.save_changes(conn), ResourceProperty::PoolCategoryName)?;
        snapshot::pool_category::modification_snapshot(conn, client, &old_category, &new_category)?;
        Ok::<_, ApiError>(new_category)
    })?;
    conn.transaction(|conn| PoolCategoryInfo::new(conn, updated_category, &fields))
        .map(Json)
        .map_err(ApiError::from)
}

/// Sets given pool category as default.
///
/// All new pools created manually or automatically will have this category.
#[utoipa::path(
    put, 
    path = "/pool-category/{name}/default", 
    tag = POOL_CATEGORY_TAG,
    params(
        ("name" = String, Path, description = "Pool category name"),
        ResourceParams,
    ),
    responses(
        (status = 200, body = PoolCategoryInfo),
        (status = 403, description = "Privileges are too low"),
        (status = 404, description = "Pool category does not exist"),
    ),
)]
async fn set_default(
    State(state): State<AppState>,
    Extension(client): Extension<Client>,
    Path(name): Path<SmallString>,
    Query(params): Query<ResourceParams>,
) -> ApiResult<Json<PoolCategoryInfo>> {
    api::verify_privilege(client, state.config.privileges().pool_category_set_default)?;

    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    let mut conn = state.get_connection()?;
    let new_default_category: PoolCategory = conn.transaction(|conn| {
        let mut category: PoolCategory = pool_category::table
            .filter(pool_category::name.eq(name))
            .first(conn)
            .optional()?
            .ok_or(ApiError::NotFound(ResourceType::PoolCategory))?;
        let mut old_default_category: PoolCategory =
            pool_category::table.filter(PoolCategory::default()).first(conn)?;

        let defaulted_pools: Vec<i64> = diesel::update(pool::table)
            .filter(pool::category_id.eq(category.id))
            .set(pool::category_id.eq(0))
            .returning(pool::id)
            .get_results(conn)?;
        diesel::update(pool::table)
            .filter(pool::category_id.eq(0))
            .filter(pool::id.ne_all(defaulted_pools))
            .set(pool::category_id.eq(category.id))
            .execute(conn)?;

        // Update last_edit_time
        let current_time = DateTime::now();
        category.last_edit_time = current_time;
        old_default_category.last_edit_time = current_time;

        // Make category default
        std::mem::swap(&mut category.id, &mut old_default_category.id);

        // Give new default category an empty name so it doesn't violate uniqueness
        let mut temporary_category_name = SmallString::new("");
        std::mem::swap(&mut category.name, &mut temporary_category_name);
        let mut new_default_category: PoolCategory = category.save_changes(conn)?;

        // Update what used to be default category
        let _: PoolCategory = old_default_category.save_changes(conn)?;

        // Give new default category back it's name
        new_default_category.name = temporary_category_name;
        let _: PoolCategory = new_default_category.save_changes(conn)?;

        snapshot::pool_category::set_default_snapshot(conn, client, &old_default_category, &new_default_category)?;
        Ok::<_, ApiError>(new_default_category)
    })?;
    conn.transaction(|conn| PoolCategoryInfo::new(conn, new_default_category, &fields))
        .map(Json)
        .map_err(ApiError::from)
}

/// Deletes an existing non-default pool category.
///
/// Pools belonging to this category will be moved to the default category.
#[utoipa::path(
    delete, 
    path = "/pool-category/{name}", 
    tag = POOL_CATEGORY_TAG,
    params(
        ("name" = String, Path, description = "Pool category name"),
    ),
    request_body = DeleteBody,
    responses(
        (status = 200, body = ()),
        (status = 400, description = "Pool category is the default category"),
        (status = 403, description = "Privileges are too low"),
        (status = 404, description = "Pool category does not exist"),
        (status = 409, description = "Version is outdated"),
    ),
)]
async fn delete(
    State(state): State<AppState>,
    Extension(client): Extension<Client>,
    Path(name): Path<SmallString>,
    Json(client_version): Json<DeleteBody>,
) -> ApiResult<Json<()>> {
    api::verify_privilege(client, state.config.privileges().pool_category_delete)?;

    state.get_connection()?.transaction(|conn| {
        let category: PoolCategory = pool_category::table
            .filter(pool_category::name.eq(name))
            .first(conn)
            .optional()?
            .ok_or(ApiError::NotFound(ResourceType::PoolCategory))?;
        api::verify_version(category.last_edit_time, *client_version)?;
        if category.id == 0 {
            return Err(ApiError::DeleteDefault(ResourceType::PoolCategory));
        }

        diesel::delete(pool_category::table.find(category.id)).execute(conn)?;
        snapshot::pool_category::deletion_snapshot(conn, client, &category)?;
        Ok(Json(()))
    })
}

#[cfg(test)]
mod test {
    use crate::api::error::ApiResult;
    use crate::model::enums::ResourceType;
    use crate::model::pool_category::PoolCategory;
    use crate::schema::{pool_category, pool_category_statistics};
    use crate::string::SmallString;
    use crate::test::*;
    use crate::time::DateTime;
    use diesel::{ExpressionMethods, PgConnection, QueryDsl, QueryResult, RunQueryDsl};
    use serial_test::{parallel, serial};

    // Exclude fields that involve creation_time or last_edit_time
    const FIELDS: &str = "&fields=name,color,usages,default";

    #[tokio::test]
    #[parallel]
    async fn list() -> ApiResult<()> {
        verify_response(&format!("GET /pool-categories/?{FIELDS}"), "pool_category/list").await
    }

    #[tokio::test]
    #[parallel]
    async fn get() -> ApiResult<()> {
        const NAME: &str = "Setting";
        let get_last_edit_time = |conn: &mut PgConnection| -> QueryResult<DateTime> {
            pool_category::table
                .select(pool_category::last_edit_time)
                .filter(pool_category::name.eq(NAME))
                .first(conn)
        };

        let mut conn = get_connection()?;
        let last_edit_time = get_last_edit_time(&mut conn)?;

        verify_response(&format!("GET /pool-category/{NAME}/?{FIELDS}"), "pool_category/get").await?;

        let new_last_edit_time = get_last_edit_time(&mut conn)?;
        assert_eq!(new_last_edit_time, last_edit_time);
        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn create() -> ApiResult<()> {
        let mut conn = get_connection()?;
        let category_count: i64 = pool_category::table.count().first(&mut conn)?;

        verify_response(&format!("POST /pool-categories/?{FIELDS}"), "pool_category/create").await?;

        let category_name: SmallString = pool_category::table
            .select(pool_category::name)
            .order_by(pool_category::id.desc())
            .first(&mut conn)?;

        let new_category_count: i64 = pool_category::table.count().first(&mut conn)?;
        let usage_count: i64 = pool_category::table
            .inner_join(pool_category_statistics::table)
            .select(pool_category_statistics::usage_count)
            .filter(pool_category::name.eq(&category_name))
            .first(&mut conn)?;
        assert_eq!(new_category_count, category_count + 1);
        assert_eq!(usage_count, 0);

        verify_response(&format!("DELETE /pool-category/{category_name}"), "pool_category/delete").await?;

        let new_category_count: i64 = pool_category::table.count().first(&mut conn)?;
        assert_eq!(new_category_count, category_count);
        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn update() -> ApiResult<()> {
        const NAME: &str = "Style";

        let mut conn = get_connection()?;
        let category: PoolCategory = pool_category::table
            .filter(pool_category::name.eq(NAME))
            .first(&mut conn)?;

        verify_response(&format!("PUT /pool-category/{NAME}/?{FIELDS}"), "pool_category/edit").await?;

        let updated_category: PoolCategory = pool_category::table
            .filter(pool_category::id.eq(category.id))
            .first(&mut conn)?;
        assert_ne!(updated_category.name, category.name);
        assert_ne!(updated_category.color, category.color);
        assert!(updated_category.last_edit_time > category.last_edit_time);

        let new_name = updated_category.name;
        verify_response(&format!("PUT /pool-category/{new_name}/?{FIELDS}"), "pool_category/edit_restore").await
    }

    #[tokio::test]
    #[serial]
    async fn set_default() -> ApiResult<()> {
        const NAME: &str = "Setting";
        let is_default = |conn: &mut PgConnection| -> QueryResult<bool> {
            let category_id: i64 = pool_category::table
                .select(pool_category::id)
                .filter(pool_category::name.eq(NAME))
                .first(conn)?;
            Ok(category_id == 0)
        };

        verify_response(&format!("PUT /pool-category/{NAME}/default/?{FIELDS}"), "pool_category/set_default").await?;

        let mut conn = get_connection()?;
        let default = is_default(&mut conn)?;
        assert!(default);

        verify_response(&format!("PUT /pool-category/default/default/?{FIELDS}"), "pool_category/restore_default")
            .await?;

        let default = is_default(&mut conn)?;
        assert!(!default);
        Ok(())
    }

    #[tokio::test]
    #[parallel]
    async fn error() -> ApiResult<()> {
        verify_response("GET /pool-category/none", "pool_category/get_nonexistent").await?;
        verify_response("PUT /pool-category/none", "pool_category/edit_nonexistent").await?;
        verify_response("PUT /pool-category/none/default", "pool_category/default_nonexistent").await?;
        verify_response("DELETE /pool-category/none", "pool_category/delete_nonexistent").await?;

        verify_response("POST /pool-categories", "pool_category/create_invalid").await?;
        verify_response("POST /pool-categories", "pool_category/create_name_clash").await?;
        verify_response("PUT /pool-category/default", "pool_category/edit_invalid").await?;
        verify_response("PUT /pool-category/default", "pool_category/edit_name_clash").await?;
        verify_response("DELETE /pool-category/default", "pool_category/delete_default").await?;

        reset_sequence(ResourceType::PoolCategory)?;
        Ok(())
    }
}
