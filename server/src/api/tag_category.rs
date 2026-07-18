use crate::api::doc::TAG_CATEGORY_TAG;
use crate::api::error::{self, ApiError, ApiResult};
use crate::app::{AppState, Context};
use crate::config::{Action, RegexType};
use crate::extract::{Ctx, DeleteBody, Json, Path, Query, ResourceParams, UnpagedResponse};
use crate::model::enums::{ResourceProperty, ResourceType};
use crate::model::tag_category::{NewTagCategory, TagCategory};
use crate::resource::tag_category::{Field, TagCategoryInfo};
use crate::schema::{tag, tag_category};
use crate::string::SmallString;
use crate::time::DateTime;
use crate::{api, snapshot};
use diesel::{ExpressionMethods, Insertable, OptionalExtension, PgConnection, QueryDsl, RunQueryDsl, SaveChangesDsl};
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

fn verify_visibility(conn: &mut PgConnection, ctx: &Context, category_name: &SmallString) -> ApiResult<TagCategory> {
    let category = tag_category::table
        .filter(tag_category::name.eq(category_name))
        .first(conn)
        .optional()?
        .ok_or(ApiError::NotFound(ResourceType::TagCategory))?;

    if ctx.preferences().category_hidden(conn, category_name)? {
        Err(ApiError::Hidden(ResourceType::TagCategory))
    } else {
        Ok(category)
    }
}

/// Lists all tag categories.
///
/// Doesn't use paging.
#[utoipa::path(
    get,
    path = "/tag-categories",
    tag = TAG_CATEGORY_TAG,
    params(ResourceParams),
    responses(
        (status = 200, body = UnpagedResponse<TagCategoryInfo>),
        (status = 403, description = "Privileges are too low"),
    ),
)]
async fn list(
    Ctx(ctx, connection_pool): Ctx,
    Query(params): Query<ResourceParams<Field>>,
) -> ApiResult<Json<UnpagedResponse<TagCategoryInfo>>> {
    ctx.verify_privilege(Action::TagCategoryView)?;
    ctx.verify_privilege(Action::TagCategoryList)?;

    connection_pool
        .transaction(move |conn| TagCategoryInfo::all(conn, &ctx, params.fields))
        .await
        .map(|results| UnpagedResponse { results })
        .map(Json)
}

/// Retrieves information about an existing tag category.
#[utoipa::path(
    get,
    path = "/tag-category/{name}",
    tag = TAG_CATEGORY_TAG,
    params(
        ("name" = String, Path, description = "Tag category name"),
        ResourceParams,
    ),
    responses(
        (status = 200, body = TagCategoryInfo),
        (status = 403, description = "Privileges are too low"),
        (status = 403, description = "Tag category is hidden"),
        (status = 404, description = "Tag category does not exist"),
    ),
)]
async fn get(
    Ctx(ctx, connection_pool): Ctx,
    Path(name): Path<SmallString>,
    Query(params): Query<ResourceParams<Field>>,
) -> ApiResult<Json<TagCategoryInfo>> {
    ctx.verify_privilege(Action::TagCategoryView)?;

    connection_pool
        .transaction(move |conn| {
            let category = verify_visibility(conn, &ctx, &name)?;
            TagCategoryInfo::new(conn, category, params.fields)
                .map(Json)
                .map_err(ApiError::from)
        })
        .await
}

/// Request body for creating a tag category.
#[derive(Deserialize, ToSchema)]
struct TagCategoryCreateBody {
    /// Display order for the category.
    order: i32,
    /// Category name. Must match `tag_category_name_regex` from server's configuration.
    name: SmallString,
    /// Category color.
    color: SmallString,
}

/// Creates a new tag category using specified parameters.
///
/// Names are case insensitive.
#[utoipa::path(
    post,
    path = "/tag-categories",
    tag = TAG_CATEGORY_TAG,
    params(ResourceParams),
    request_body = TagCategoryCreateBody,
    responses(
        (status = 200, body = TagCategoryInfo),
        (status = 403, description = "Privileges are too low"),
        (status = 409, description = "Name is used by an existing tag category"),
        (status = 422, description = "Name is invalid or missing"),
        (status = 422, description = "Color is invalid or missing"),
    ),
)]
async fn create(
    Ctx(ctx, connection_pool): Ctx,
    Query(params): Query<ResourceParams<Field>>,
    Json(body): Json<TagCategoryCreateBody>,
) -> ApiResult<Json<TagCategoryInfo>> {
    ctx.verify_privilege(Action::TagCategoryCreate)?;
    api::verify_matches_regex(&ctx.config, &body.name, RegexType::TagCategory)?;

    let category = connection_pool
        .transaction(move |conn| {
            let category = NewTagCategory {
                order: body.order,
                name: &body.name,
                color: &body.color,
            }
            .insert_into(tag_category::table)
            .on_conflict(tag_category::name)
            .do_nothing()
            .get_result(conn)
            .optional()?
            .ok_or(ApiError::AlreadyExists(ResourceProperty::TagCategoryName))?;
            snapshot::tag_category::creation_snapshot(conn, ctx.client, &category)?;
            Ok::<_, ApiError>(category)
        })
        .await?;
    connection_pool
        .transaction(move |conn| TagCategoryInfo::new(conn, category, params.fields))
        .await
        .map(Json)
}

/// Request body for updating a tag category.
#[derive(Deserialize, ToSchema)]
struct TagCategoryUpdateBody {
    /// Resource version. See [versioning](#Versioning).
    version: DateTime,
    /// Display order for the category.
    order: Option<i32>,
    /// New category name. Must match `tag_category_name_regex` from server's configuration.
    name: Option<SmallString>,
    /// New category color.
    color: Option<SmallString>,
}

/// Updates an existing tag category using specified parameters.
///
/// Name must match `tag_category_name_regex` from server's configuration.
/// Names are case insensitive. All fields except `version` are optional -
/// update concerns only provided fields.
#[utoipa::path(
    put,
    path = "/tag-category/{name}",
    tag = TAG_CATEGORY_TAG,
    params(
        ("name" = String, Path, description = "Tag category name"),
        ResourceParams,
    ),
    request_body = TagCategoryUpdateBody,
    responses(
        (status = 200, body = TagCategoryInfo),
        (status = 403, description = "Privileges are too low"),
        (status = 404, description = "Tag category does not exist"),
        (status = 409, description = "Version is outdated"),
        (status = 409, description = "Name is used by an existing tag category"),
        (status = 422, description = "Name is invalid"),
        (status = 422, description = "Color is invalid"),
    ),
)]
async fn update(
    Ctx(ctx, connection_pool): Ctx,
    Path(name): Path<SmallString>,
    Query(params): Query<ResourceParams<Field>>,
    Json(body): Json<TagCategoryUpdateBody>,
) -> ApiResult<Json<TagCategoryInfo>> {
    ctx.verify_privilege(Action::TagCategoryView)?;

    let updated_category = connection_pool
        .transaction(move |conn| {
            let old_category: TagCategory = tag_category::table
                .filter(tag_category::name.eq(name))
                .first(conn)
                .optional()?
                .ok_or(ApiError::NotFound(ResourceType::TagCategory))?;
            api::verify_version(old_category.last_edit_time, body.version)?;

            let mut new_category = old_category.clone();
            if let Some(order) = body.order {
                ctx.verify_privilege(Action::TagCategoryEditOrder)?;
                new_category.order = order;
            }
            if let Some(name) = body.name {
                ctx.verify_privilege(Action::TagCategoryEditName)?;
                api::verify_matches_regex(&ctx.config, &name, RegexType::TagCategory)?;
                new_category.name = name;
            }
            if let Some(color) = body.color {
                ctx.verify_privilege(Action::TagCategoryEditColor)?;
                new_category.color = color;
            }

            new_category.last_edit_time = DateTime::now();
            let saved_category: TagCategory =
                error::map_unique_violation(new_category.save_changes(conn), ResourceProperty::TagCategoryName)?;
            snapshot::tag_category::modification_snapshot(conn, ctx.client, &old_category, &new_category)?;
            Ok::<_, ApiError>(saved_category)
        })
        .await?;
    connection_pool
        .transaction(move |conn| TagCategoryInfo::new(conn, updated_category, params.fields))
        .await
        .map(Json)
}

/// Sets given tag category as default.
///
/// All new tags created manually or automatically will have this category.
#[utoipa::path(
    put,
    path = "/tag-category/{name}/default",
    tag = TAG_CATEGORY_TAG,
    params(
        ("name" = String, Path, description = "Tag category name"),
        ResourceParams,
    ),
    request_body = Object,
    responses(
        (status = 200, body = TagCategoryInfo),
        (status = 403, description = "Privileges are too low"),
        (status = 404, description = "Tag category does not exist"),
    ),
)]
async fn set_default(
    Ctx(ctx, connection_pool): Ctx,
    Path(name): Path<SmallString>,
    Query(params): Query<ResourceParams<Field>>,
) -> ApiResult<Json<TagCategoryInfo>> {
    ctx.verify_privilege(Action::TagCategoryView)?;
    ctx.verify_privilege(Action::TagCategorySetDefault)?;

    let new_default_category: TagCategory = connection_pool
        .transaction(move |conn| {
            let mut category: TagCategory = tag_category::table
                .filter(tag_category::name.eq(name))
                .first(conn)
                .optional()?
                .ok_or(ApiError::NotFound(ResourceType::TagCategory))?;
            let mut old_default_category: TagCategory =
                tag_category::table.filter(TagCategory::is_default()).first(conn)?;

            let defaulted_tags: Vec<i64> = diesel::update(tag::table)
                .filter(tag::category_id.eq(category.id))
                .set(tag::category_id.eq(0))
                .returning(tag::id)
                .get_results(conn)?;
            diesel::update(tag::table)
                .filter(tag::category_id.eq(0))
                .filter(tag::id.ne_all(defaulted_tags))
                .set(tag::category_id.eq(category.id))
                .execute(conn)?;

            // Make category default
            std::mem::swap(&mut category.id, &mut old_default_category.id);

            // Give new default category an empty name so it doesn't violate uniqueness
            let mut temporary_category_name = SmallString::new("");
            std::mem::swap(&mut category.name, &mut temporary_category_name);
            let mut new_default_category: TagCategory = category.save_changes(conn)?;

            // Update what used to be default category
            let _: TagCategory = old_default_category.save_changes(conn)?;

            // Give new default category back it's name
            new_default_category.name = temporary_category_name;
            let _: TagCategory = new_default_category.save_changes(conn)?;

            snapshot::tag_category::set_default_snapshot(
                conn,
                ctx.client,
                &old_default_category,
                &new_default_category,
            )?;
            Ok::<_, ApiError>(new_default_category)
        })
        .await?;
    connection_pool
        .transaction(move |conn| TagCategoryInfo::new(conn, new_default_category, params.fields))
        .await
        .map(Json)
}

/// Deletes an existing non-default tag category.
///
/// Tags belonging to this category will be moved to the default category.
#[utoipa::path(
    delete,
    path = "/tag-category/{name}",
    tag = TAG_CATEGORY_TAG,
    params(
        ("name" = String, Path, description = "Tag category name"),
    ),
    request_body = DeleteBody,
    responses(
        (status = 200, body = Object),
        (status = 403, description = "Privileges are too low"),
        (status = 404, description = "Tag category does not exist"),
        (status = 409, description = "Version is outdated"),
        (status = 422, description = "Tag category is the default category"),
    ),
)]
async fn delete(
    Ctx(ctx, connection_pool): Ctx,
    Path(name): Path<SmallString>,
    Json(client_version): Json<DeleteBody>,
) -> ApiResult<Json<()>> {
    ctx.verify_privilege(Action::TagCategoryDelete)?;

    connection_pool
        .transaction(move |conn| {
            let category: TagCategory = tag_category::table
                .filter(tag_category::name.eq(name))
                .first(conn)
                .optional()?
                .ok_or(ApiError::NotFound(ResourceType::TagCategory))?;

            if category.id == 0 {
                return Err(ApiError::DeleteDefault(ResourceType::TagCategory));
            }
            api::verify_version(category.last_edit_time, *client_version)?;

            diesel::delete(tag_category::table.find(category.id)).execute(conn)?;
            snapshot::tag_category::deletion_snapshot(conn, ctx.client, &category)?;
            Ok(Json(()))
        })
        .await
}

#[cfg(test)]
mod test {
    use crate::api::error::ApiResult;
    use crate::model::enums::{ResourceType, UserRank};
    use crate::model::tag_category::TagCategory;
    use crate::schema::{tag_category, tag_category_statistics};
    use crate::test::*;
    use crate::time::DateTime;
    use diesel::{ExpressionMethods, PgConnection, QueryDsl, QueryResult, RunQueryDsl};
    use serial_test::{parallel, serial};

    // Exclude fields that involve creation_time or last_edit_time
    const FIELDS: &str = "&fields=name,color,usages,order,default";

    #[tokio::test]
    #[parallel]
    async fn list() -> ApiResult<()> {
        verify_response(&format!("GET /tag-categories/?{FIELDS}"), "tag_category/list/typical").await
    }

    #[tokio::test]
    #[parallel]
    async fn get() -> ApiResult<()> {
        const NAME: &str = "Source";
        let get_last_edit_time = |conn: &mut PgConnection| -> QueryResult<DateTime> {
            tag_category::table
                .select(tag_category::last_edit_time)
                .filter(tag_category::name.eq(NAME))
                .first(conn)
        };

        let mut conn = get_connection()?;
        let last_edit_time = get_last_edit_time(&mut conn)?;

        verify_response(&format!("GET /tag-category/{NAME}/?{FIELDS}"), "tag_category/get/typical").await?;

        let new_last_edit_time = get_last_edit_time(&mut conn)?;
        assert_eq!(new_last_edit_time, last_edit_time);
        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn create() -> ApiResult<()> {
        let mut conn = get_connection()?;
        let category_count: i64 = tag_category::table.count().first(&mut conn)?;

        verify_response(&format!("POST /tag-categories/?{FIELDS}"), "tag_category/create/typical").await?;

        let category_name: String = tag_category::table
            .select(tag_category::name)
            .order(tag_category::id.desc())
            .first(&mut conn)?;

        let new_category_count: i64 = tag_category::table.count().first(&mut conn)?;
        let usage_count: i64 = tag_category::table
            .inner_join(tag_category_statistics::table)
            .select(tag_category_statistics::usage_count)
            .filter(tag_category::name.eq(&category_name))
            .first(&mut conn)?;
        assert_eq!(new_category_count, category_count + 1);
        assert_eq!(usage_count, 0);

        verify_response(&format!("DELETE /tag-category/{category_name}"), "tag_category/delete/typical").await?;

        let new_category_count: i64 = tag_category::table.count().first(&mut conn)?;
        assert_eq!(new_category_count, category_count);
        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn update() -> ApiResult<()> {
        const NAME: &str = "Character";

        let mut conn = get_connection()?;
        let category: TagCategory = tag_category::table
            .filter(tag_category::name.eq(NAME))
            .first(&mut conn)?;

        verify_response(&format!("PUT /tag-category/{NAME}/?{FIELDS}"), "tag_category/edit/typical").await?;

        let updated_category: TagCategory = tag_category::table
            .filter(tag_category::id.eq(category.id))
            .first(&mut conn)?;
        assert_ne!(updated_category.name, category.name);
        assert_ne!(updated_category.color, category.color);
        assert!(updated_category.last_edit_time > category.last_edit_time);

        let new_name = updated_category.name;
        verify_response(&format!("PUT /tag-category/{new_name}/?{FIELDS}"), "tag_category/edit/restore").await
    }

    #[tokio::test]
    #[serial]
    async fn set_default() -> ApiResult<()> {
        const NAME: &str = "Surroundings";
        let is_default = |conn: &mut PgConnection| -> QueryResult<bool> {
            let category_id: i64 = tag_category::table
                .select(tag_category::id)
                .filter(tag_category::name.eq(NAME))
                .first(conn)?;
            Ok(category_id == 0)
        };

        verify_response(&format!("PUT /tag-category/{NAME}/default/?{FIELDS}"), "tag_category/set_default/typical")
            .await?;

        let mut conn = get_connection()?;
        let default = is_default(&mut conn)?;
        assert!(default);

        verify_response(&format!("PUT /tag-category/default/default/?{FIELDS}"), "tag_category/set_default/restore")
            .await?;

        let default = is_default(&mut conn)?;
        assert!(!default);
        Ok(())
    }

    #[tokio::test]
    #[parallel]
    async fn preferences() -> ApiResult<()> {
        verify_response_with_user(
            UserRank::Anonymous,
            "GET /tag-categories/?fields=name",
            "tag_category/list/with_preferences",
        )
        .await?;
        verify_response_with_user(UserRank::Anonymous, "GET /tag-category/meta", "tag_category/get/with_preferences")
            .await
    }

    #[tokio::test]
    #[parallel]
    async fn error() -> ApiResult<()> {
        verify_response("GET /tag-category/none", "tag_category/get/nonexistent").await?;
        verify_response("PUT /tag-category/none", "tag_category/edit/nonexistent").await?;
        verify_response("PUT /tag-category/none/default", "tag_category/set_default/nonexistent").await?;
        verify_response("DELETE /tag-category/none", "tag_category/delete/nonexistent").await?;

        verify_response("POST /tag-categories", "tag_category/create/invalid").await?;
        verify_response("POST /tag-categories", "tag_category/create/name_clash").await?;
        verify_response("PUT /tag-category/default", "tag_category/edit/invalid").await?;
        verify_response("PUT /tag-category/default", "tag_category/edit/name_clash").await?;
        verify_response("DELETE /tag-category/default", "tag_category/delete/default").await?;

        reset_sequence(ResourceType::PoolCategory)
    }

    #[tokio::test]
    #[parallel]
    async fn unauthorized() -> ApiResult<()> {
        const USER: UserRank = UserRank::Regular;

        verify_response_with_user(USER, "GET /tag-categories?limit=1", "tag_category/list/unauthorized").await?;
        verify_response_with_user(USER, "GET /tag-category/default", "tag_category/get/unauthorized").await?;
        verify_response_with_user(USER, "PUT /tag-category/default", "tag_category/edit/name_unauthorized").await?;
        verify_response_with_user(USER, "PUT /tag-category/default", "tag_category/edit/color_unauthorized").await?;
        verify_response_with_user(USER, "PUT /tag-category/default", "tag_category/edit/order_unauthorized").await?;
        verify_response_with_user(USER, "PUT /tag-category/meta/default", "tag_category/set_default/unauthorized")
            .await?;
        verify_response_with_user(USER, "DELETE /tag-category/meta", "tag_category/delete/unauthorized").await?;

        // Ensure users can't get around lack of view privileges via other actions
        verify_response_with_user(USER, "GET /tag-categories?limit=1", "tag_category/list/view_unauthorized").await?;
        verify_response_with_user(USER, "PUT /tag-category/default", "tag_category/edit/view_unauthorized").await?;
        verify_response_with_user(USER, "PUT /tag-category/meta/default", "tag_category/set_default/view_unauthorized")
            .await
    }

    #[tokio::test]
    #[serial]
    async fn blacklist_edge_case() -> ApiResult<()> {
        // Create edge-case tag category name
        verify_response("POST /tag-categories?fields=name", "tag_category/create/blacklist_edge_case").await?;

        // Try to view tag category using name with different casing
        verify_response_with_user(
            UserRank::Anonymous,
            "GET /tag-category/κοσμοσ",
            "tag_category/get/blacklist_edge_case",
        )
        .await?;

        reset_database();
        Ok(())
    }
}
