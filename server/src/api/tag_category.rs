use crate::api::error::{ApiError, ApiResult};
use crate::api::extract::{Json, Path, Query};
use crate::api::{DeleteBody, ResourceParams, UnpagedResponse, error};
use crate::app::AppState;
use crate::auth::Client;
use crate::config::RegexType;
use crate::model::enums::{ResourceProperty, ResourceType};
use crate::model::tag_category::{NewTagCategory, TagCategory};
use crate::resource::tag_category::TagCategoryInfo;
use crate::schema::{tag, tag_category};
use crate::string::SmallString;
use crate::time::DateTime;
use crate::{api, resource, snapshot};
use axum::extract::{Extension, State};
use axum::{Router, routing};
use diesel::{Connection, ExpressionMethods, Insertable, OptionalExtension, QueryDsl, RunQueryDsl, SaveChangesDsl};
use serde::Deserialize;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/tag-categories", routing::get(list).post(create))
        .route("/tag-category/{name}", routing::get(get).put(update).delete(delete))
        .route("/tag-category/{name}/default", routing::put(set_default))
}

/// See [listing-tag-categories](https://github.com/liamw1/oxibooru/blob/master/docs/API.md#listing-tag-categories)
async fn list(
    State(state): State<AppState>,
    Extension(client): Extension<Client>,
    Query(params): Query<ResourceParams>,
) -> ApiResult<Json<UnpagedResponse<TagCategoryInfo>>> {
    api::verify_privilege(client, state.config.privileges().tag_category_list)?;

    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    state
        .get_connection()?
        .transaction(|conn| TagCategoryInfo::all(conn, &fields))
        .map(|results| UnpagedResponse { results })
        .map(Json)
        .map_err(ApiError::from)
}

/// See [getting-tag-category](https://github.com/liamw1/oxibooru/blob/master/docs/API.md#getting-tag-category)
async fn get(
    State(state): State<AppState>,
    Extension(client): Extension<Client>,
    Path(name): Path<String>,
    Query(params): Query<ResourceParams>,
) -> ApiResult<Json<TagCategoryInfo>> {
    api::verify_privilege(client, state.config.privileges().tag_category_view)?;

    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    state.get_connection()?.transaction(|conn| {
        let category = tag_category::table
            .filter(tag_category::name.eq(name))
            .first(conn)
            .optional()?
            .ok_or(ApiError::NotFound(ResourceType::TagCategory))?;
        TagCategoryInfo::new(conn, category, &fields)
            .map(Json)
            .map_err(ApiError::from)
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateBody {
    order: i32,
    name: SmallString,
    color: SmallString,
}

/// See [creating-tag-category](https://github.com/liamw1/oxibooru/blob/master/docs/API.md#creating-tag-category)
async fn create(
    State(state): State<AppState>,
    Extension(client): Extension<Client>,
    Query(params): Query<ResourceParams>,
    Json(body): Json<CreateBody>,
) -> ApiResult<Json<TagCategoryInfo>> {
    api::verify_privilege(client, state.config.privileges().tag_category_create)?;
    api::verify_matches_regex(&state.config, &body.name, RegexType::TagCategory)?;
    let fields = resource::create_table(params.fields()).map_err(Box::from)?;

    let new_category = NewTagCategory {
        order: body.order,
        name: &body.name,
        color: &body.color,
    };

    let mut conn = state.get_connection()?;
    let category = conn.transaction(|conn| {
        let category = new_category
            .insert_into(tag_category::table)
            .on_conflict(tag_category::name)
            .do_nothing()
            .get_result(conn)
            .optional()?
            .ok_or(ApiError::AlreadyExists(ResourceProperty::TagCategoryName))?;
        snapshot::tag_category::creation_snapshot(conn, client, &category)?;
        Ok::<_, ApiError>(category)
    })?;
    conn.transaction(|conn| TagCategoryInfo::new(conn, category, &fields))
        .map(Json)
        .map_err(ApiError::from)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdateBody {
    version: DateTime,
    order: Option<i32>,
    name: Option<SmallString>,
    color: Option<SmallString>,
}

/// See [updating-tag-category](https://github.com/liamw1/oxibooru/blob/master/docs/API.md#updating-tag-category)
async fn update(
    State(state): State<AppState>,
    Extension(client): Extension<Client>,
    Path(name): Path<String>,
    Query(params): Query<ResourceParams>,
    Json(body): Json<UpdateBody>,
) -> ApiResult<Json<TagCategoryInfo>> {
    let fields = resource::create_table(params.fields()).map_err(Box::from)?;

    let mut conn = state.get_connection()?;
    let updated_category = conn.transaction(|conn| {
        let old_category: TagCategory = tag_category::table
            .filter(tag_category::name.eq(name))
            .first(conn)
            .optional()?
            .ok_or(ApiError::NotFound(ResourceType::TagCategory))?;
        api::verify_version(old_category.last_edit_time, body.version)?;

        let mut new_category = old_category.clone();
        if let Some(order) = body.order {
            api::verify_privilege(client, state.config.privileges().tag_category_edit_order)?;
            new_category.order = order;
        }
        if let Some(name) = body.name {
            api::verify_privilege(client, state.config.privileges().tag_category_edit_name)?;
            api::verify_matches_regex(&state.config, &name, RegexType::TagCategory)?;
            new_category.name = name;
        }
        if let Some(color) = body.color {
            api::verify_privilege(client, state.config.privileges().tag_category_edit_color)?;
            new_category.color = color;
        }

        new_category.last_edit_time = DateTime::now();
        let _: TagCategory =
            error::map_unique_violation(new_category.save_changes(conn), ResourceProperty::TagCategoryName)?;
        snapshot::tag_category::modification_snapshot(conn, client, &old_category, &new_category)?;
        Ok::<_, ApiError>(new_category)
    })?;
    conn.transaction(|conn| TagCategoryInfo::new(conn, updated_category, &fields))
        .map(Json)
        .map_err(ApiError::from)
}

/// See [setting-default-tag-category](https://github.com/liamw1/oxibooru/blob/master/docs/API.md#setting-default-tag-category)
async fn set_default(
    State(state): State<AppState>,
    Extension(client): Extension<Client>,
    Path(name): Path<String>,
    Query(params): Query<ResourceParams>,
) -> ApiResult<Json<TagCategoryInfo>> {
    api::verify_privilege(client, state.config.privileges().tag_category_set_default)?;

    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    let mut conn = state.get_connection()?;
    let new_default_category: TagCategory = conn.transaction(|conn| {
        let mut category: TagCategory = tag_category::table
            .filter(tag_category::name.eq(name))
            .first(conn)
            .optional()?
            .ok_or(ApiError::NotFound(ResourceType::TagCategory))?;
        let mut old_default_category: TagCategory = tag_category::table.filter(TagCategory::default()).first(conn)?;

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

        // Update last_edit_time
        let current_time = DateTime::now();
        category.last_edit_time = current_time;
        old_default_category.last_edit_time = current_time;

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

        snapshot::tag_category::set_default_snapshot(conn, client, &old_default_category, &new_default_category)?;
        Ok::<_, ApiError>(new_default_category)
    })?;
    conn.transaction(|conn| TagCategoryInfo::new(conn, new_default_category, &fields))
        .map(Json)
        .map_err(ApiError::from)
}

/// See [deleting-tag-category](https://github.com/liamw1/oxibooru/blob/master/docs/API.md#deleting-tag-category)
async fn delete(
    State(state): State<AppState>,
    Extension(client): Extension<Client>,
    Path(name): Path<String>,
    Json(client_version): Json<DeleteBody>,
) -> ApiResult<Json<()>> {
    api::verify_privilege(client, state.config.privileges().tag_category_delete)?;

    state.get_connection()?.transaction(|conn| {
        let category: TagCategory = tag_category::table
            .filter(tag_category::name.eq(name))
            .first(conn)
            .optional()?
            .ok_or(ApiError::NotFound(ResourceType::TagCategory))?;
        api::verify_version(category.last_edit_time, *client_version)?;
        if category.id == 0 {
            return Err(ApiError::DeleteDefault(ResourceType::TagCategory));
        }

        diesel::delete(tag_category::table.find(category.id)).execute(conn)?;
        snapshot::tag_category::deletion_snapshot(conn, client, &category)?;
        Ok(Json(()))
    })
}

#[cfg(test)]
mod test {
    use crate::api::error::ApiResult;
    use crate::model::enums::ResourceType;
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
        verify_response(&format!("GET /tag-categories/?{FIELDS}"), "tag_category/list").await
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

        verify_response(&format!("GET /tag-category/{NAME}/?{FIELDS}"), "tag_category/get").await?;

        let new_last_edit_time = get_last_edit_time(&mut conn)?;
        assert_eq!(new_last_edit_time, last_edit_time);
        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn create() -> ApiResult<()> {
        let mut conn = get_connection()?;
        let category_count: i64 = tag_category::table.count().first(&mut conn)?;

        verify_response(&format!("POST /tag-categories/?{FIELDS}"), "tag_category/create").await?;

        let category_name: String = tag_category::table
            .select(tag_category::name)
            .order_by(tag_category::id.desc())
            .first(&mut conn)?;

        let new_category_count: i64 = tag_category::table.count().first(&mut conn)?;
        let usage_count: i64 = tag_category::table
            .inner_join(tag_category_statistics::table)
            .select(tag_category_statistics::usage_count)
            .filter(tag_category::name.eq(&category_name))
            .first(&mut conn)?;
        assert_eq!(new_category_count, category_count + 1);
        assert_eq!(usage_count, 0);

        verify_response(&format!("DELETE /tag-category/{category_name}"), "tag_category/delete").await?;

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

        verify_response(&format!("PUT /tag-category/{NAME}/?{FIELDS}"), "tag_category/edit").await?;

        let updated_category: TagCategory = tag_category::table
            .filter(tag_category::id.eq(category.id))
            .first(&mut conn)?;
        assert_ne!(updated_category.name, category.name);
        assert_ne!(updated_category.color, category.color);
        assert!(updated_category.last_edit_time > category.last_edit_time);

        let new_name = updated_category.name;
        verify_response(&format!("PUT /tag-category/{new_name}/?{FIELDS}"), "tag_category/edit_restore").await
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

        verify_response(&format!("PUT /tag-category/{NAME}/default/?{FIELDS}"), "tag_category/set_default").await?;

        let mut conn = get_connection()?;
        let default = is_default(&mut conn)?;
        assert!(default);

        verify_response(&format!("PUT /tag-category/default/default/?{FIELDS}"), "tag_category/restore_default")
            .await?;

        let default = is_default(&mut conn)?;
        assert!(!default);
        Ok(())
    }

    #[tokio::test]
    #[parallel]
    async fn error() -> ApiResult<()> {
        verify_response("GET /tag-category/none", "tag_category/get_nonexistent").await?;
        verify_response("PUT /tag-category/none", "tag_category/edit_nonexistent").await?;
        verify_response("PUT /tag-category/none/default", "tag_category/default_nonexistent").await?;
        verify_response("DELETE /tag-category/none", "tag_category/delete_nonexistent").await?;

        verify_response("POST /tag-categories", "tag_category/create_invalid").await?;
        verify_response("POST /tag-categories", "tag_category/create_name_clash").await?;
        verify_response("PUT /tag-category/default", "tag_category/edit_invalid").await?;
        verify_response("PUT /tag-category/default", "tag_category/edit_name_clash").await?;
        verify_response("DELETE /tag-category/default", "tag_category/delete_default").await?;

        reset_sequence(ResourceType::PoolCategory)?;
        Ok(())
    }
}
