use crate::api::{ApiResult, DeleteBody, ResourceParams, UnpagedResponse};
use crate::auth::Client;
use crate::config::RegexType;
use crate::model::enums::ResourceType;
use crate::model::tag_category::{NewTagCategory, TagCategory};
use crate::resource::tag_category::TagCategoryInfo;
use crate::schema::{tag, tag_category};
use crate::string::SmallString;
use crate::time::DateTime;
use crate::{api, config, db, resource, snapshot};
use axum::extract::{Extension, Path, Query};
use axum::{Json, Router, routing};
use diesel::prelude::*;
use serde::Deserialize;

pub fn routes() -> Router {
    Router::new()
        .route("/tag-categories", routing::get(list).post(create))
        .route("/tag-category/{name}", routing::get(get).put(update).delete(delete))
        .route("/tag-category/{name}/default", routing::put(set_default))
}

async fn list(
    Extension(client): Extension<Client>,
    Query(params): Query<ResourceParams>,
) -> ApiResult<Json<UnpagedResponse<TagCategoryInfo>>> {
    api::verify_privilege(client, config::privileges().tag_category_list)?;

    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    db::get_connection()?
        .transaction(|conn| TagCategoryInfo::all(conn, &fields))
        .map(|results| UnpagedResponse { results })
        .map(Json)
        .map_err(api::Error::from)
}

async fn get(
    Extension(client): Extension<Client>,
    Path(name): Path<String>,
    Query(params): Query<ResourceParams>,
) -> ApiResult<Json<TagCategoryInfo>> {
    api::verify_privilege(client, config::privileges().tag_category_view)?;

    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    db::get_connection()?.transaction(|conn| {
        let category = tag_category::table
            .filter(tag_category::name.eq(name))
            .first(conn)
            .optional()?
            .ok_or(api::Error::NotFound(ResourceType::TagCategory))?;
        TagCategoryInfo::new(conn, category, &fields)
            .map(Json)
            .map_err(api::Error::from)
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateBody {
    order: i32,
    name: SmallString,
    color: SmallString,
}

async fn create(
    Extension(client): Extension<Client>,
    Query(params): Query<ResourceParams>,
    Json(body): Json<CreateBody>,
) -> ApiResult<Json<TagCategoryInfo>> {
    api::verify_privilege(client, config::privileges().tag_category_create)?;
    api::verify_matches_regex(&body.name, RegexType::TagCategory)?;
    let fields = resource::create_table(params.fields()).map_err(Box::from)?;

    let new_category = NewTagCategory {
        order: body.order,
        name: &body.name,
        color: &body.color,
    };

    let mut conn = db::get_connection()?;
    let category = conn.transaction(|conn| {
        let category = new_category.insert_into(tag_category::table).get_result(conn)?;
        snapshot::tag_category::creation_snapshot(conn, client, &category).map(|()| category)
    })?;
    conn.transaction(|conn| TagCategoryInfo::new(conn, category, &fields))
        .map(Json)
        .map_err(api::Error::from)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdateBody {
    version: DateTime,
    order: Option<SmallString>, // TODO: Client sends order out as string so we convert on server, would be better to do this on client
    name: Option<SmallString>,
    color: Option<SmallString>,
}

async fn update(
    Extension(client): Extension<Client>,
    Path(name): Path<String>,
    Query(params): Query<ResourceParams>,
    Json(body): Json<UpdateBody>,
) -> ApiResult<Json<TagCategoryInfo>> {
    let fields = resource::create_table(params.fields()).map_err(Box::from)?;

    let mut conn = db::get_connection()?;
    let updated_category = conn.transaction(|conn| {
        let old_category: TagCategory = tag_category::table.filter(tag_category::name.eq(name)).first(conn)?;
        api::verify_version(old_category.last_edit_time, body.version)?;

        let mut new_category = old_category.clone();
        if let Some(order) = body.order {
            api::verify_privilege(client, config::privileges().tag_category_edit_order)?;
            new_category.order = order.parse::<i32>()?;
        }
        if let Some(name) = body.name {
            api::verify_privilege(client, config::privileges().tag_category_edit_name)?;
            api::verify_matches_regex(&name, RegexType::TagCategory)?;
            new_category.name = name;
        }
        if let Some(color) = body.color {
            api::verify_privilege(client, config::privileges().tag_category_edit_color)?;
            new_category.color = color;
        }

        new_category.last_edit_time = DateTime::now();
        let _: TagCategory = new_category.save_changes(conn)?;
        snapshot::tag_category::modification_snapshot(conn, client, &old_category, &new_category)?;
        Ok::<_, api::Error>(new_category)
    })?;
    conn.transaction(|conn| TagCategoryInfo::new(conn, updated_category, &fields))
        .map(Json)
        .map_err(api::Error::from)
}

async fn set_default(
    Extension(client): Extension<Client>,
    Path(name): Path<String>,
    Query(params): Query<ResourceParams>,
) -> ApiResult<Json<TagCategoryInfo>> {
    api::verify_privilege(client, config::privileges().tag_category_set_default)?;

    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    let mut conn = db::get_connection()?;
    let new_default_category: TagCategory = conn.transaction(|conn| {
        let mut category: TagCategory = tag_category::table.filter(tag_category::name.eq(name)).first(conn)?;
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
        Ok::<_, api::Error>(new_default_category)
    })?;
    conn.transaction(|conn| TagCategoryInfo::new(conn, new_default_category, &fields))
        .map(Json)
        .map_err(api::Error::from)
}

async fn delete(
    Extension(client): Extension<Client>,
    Path(name): Path<String>,
    Json(client_version): Json<DeleteBody>,
) -> ApiResult<Json<()>> {
    api::verify_privilege(client, config::privileges().tag_category_delete)?;

    db::get_connection()?.transaction(|conn| {
        let category: TagCategory = tag_category::table.filter(tag_category::name.eq(name)).first(conn)?;
        api::verify_version(category.last_edit_time, *client_version)?;
        if category.id == 0 {
            return Err(api::Error::DeleteDefault(ResourceType::TagCategory));
        }

        diesel::delete(tag_category::table.find(category.id)).execute(conn)?;
        snapshot::tag_category::deletion_snapshot(conn, client, &category)?;
        Ok(Json(()))
    })
}

#[cfg(test)]
mod test {
    use crate::api::ApiResult;
    use crate::model::tag_category::TagCategory;
    use crate::schema::{tag_category, tag_category_statistics};
    use crate::test::*;
    use crate::time::DateTime;
    use diesel::prelude::*;
    use serial_test::{parallel, serial};

    // Exclude fields that involve creation_time or last_edit_time
    const FIELDS: &str = "&fields=name,color,usages,order,default";

    #[tokio::test]
    #[parallel]
    async fn list() -> ApiResult<()> {
        verify_query(&format!("GET /tag-categories/?{FIELDS}"), "tag_category/list.json").await
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

        verify_query(&format!("GET /tag-category/{NAME}/?{FIELDS}"), "tag_category/get.json").await?;

        let new_last_edit_time = get_last_edit_time(&mut conn)?;
        assert_eq!(new_last_edit_time, last_edit_time);
        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn create() -> ApiResult<()> {
        let mut conn = get_connection()?;
        let category_count: i64 = tag_category::table.count().first(&mut conn)?;

        verify_query(&format!("POST /tag-categories/?{FIELDS}"), "tag_category/create.json").await?;

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

        verify_query(&format!("DELETE /tag-category/{category_name}"), "tag_category/delete.json").await?;

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

        verify_query(&format!("PUT /tag-category/{NAME}/?{FIELDS}"), "tag_category/update.json").await?;

        let updated_category: TagCategory = tag_category::table
            .filter(tag_category::id.eq(category.id))
            .first(&mut conn)?;
        assert_ne!(updated_category.name, category.name);
        assert_ne!(updated_category.color, category.color);
        assert!(updated_category.last_edit_time > category.last_edit_time);

        let new_name = updated_category.name;
        verify_query(&format!("PUT /tag-category/{new_name}/?{FIELDS}"), "tag_category/update_restore.json").await
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

        verify_query(&format!("PUT /tag-category/{NAME}/default/?{FIELDS}"), "tag_category/set_default.json").await?;

        let mut conn = get_connection()?;
        let default = is_default(&mut conn)?;
        assert!(default);

        verify_query(&format!("PUT /tag-category/default/default/?{FIELDS}"), "tag_category/restore_default.json")
            .await?;

        let default = is_default(&mut conn)?;
        assert!(!default);
        Ok(())
    }
}
