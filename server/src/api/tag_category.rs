use crate::api::{ApiResult, AuthResult, DeleteRequest, ResourceQuery, UnpagedResponse};
use crate::config::RegexType;
use crate::model::enums::ResourceType;
use crate::model::tag::{NewTagCategory, TagCategory};
use crate::resource::tag_category::TagCategoryInfo;
use crate::schema::{tag, tag_category};
use crate::time::DateTime;
use crate::{api, config, db, resource};
use diesel::prelude::*;
use serde::Deserialize;
use warp::{Filter, Rejection, Reply};

pub fn routes() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    let list = warp::get()
        .and(api::auth())
        .and(warp::path!("tag-categories"))
        .and(api::resource_query())
        .map(list)
        .map(api::Reply::from);
    let get = warp::get()
        .and(api::auth())
        .and(warp::path!("tag-category" / String))
        .and(api::resource_query())
        .map(get)
        .map(api::Reply::from);
    let create = warp::post()
        .and(api::auth())
        .and(warp::path!("tag-categories"))
        .and(api::resource_query())
        .and(warp::body::json())
        .map(create)
        .map(api::Reply::from);
    let update = warp::put()
        .and(api::auth())
        .and(warp::path!("tag-category" / String))
        .and(api::resource_query())
        .and(warp::body::json())
        .map(update)
        .map(api::Reply::from);
    let set_default = warp::put()
        .and(api::auth())
        .and(warp::path!("tag-category" / String / "default"))
        .and(api::resource_query())
        .map(set_default)
        .map(api::Reply::from);
    let delete = warp::delete()
        .and(api::auth())
        .and(warp::path!("tag-category" / String))
        .and(warp::body::json())
        .map(delete)
        .map(api::Reply::from);

    list.or(get).or(create).or(update).or(set_default).or(delete)
}

fn list(auth: AuthResult, query: ResourceQuery) -> ApiResult<UnpagedResponse<TagCategoryInfo>> {
    let client = auth?;
    api::verify_privilege(client, config::privileges().tag_category_list)?;

    let fields = resource::create_table(query.fields()).map_err(Box::from)?;
    db::get_connection()?.transaction(|conn| {
        TagCategoryInfo::all(conn, &fields)
            .map(|results| UnpagedResponse { results })
            .map_err(api::Error::from)
    })
}

fn get(auth: AuthResult, name: String, query: ResourceQuery) -> ApiResult<TagCategoryInfo> {
    let client = auth?;
    api::verify_privilege(client, config::privileges().tag_category_view)?;

    let fields = resource::create_table(query.fields()).map_err(Box::from)?;
    let name = percent_encoding::percent_decode_str(&name).decode_utf8()?;
    db::get_connection()?.transaction(|conn| {
        let category = tag_category::table
            .filter(tag_category::name.eq(name))
            .first(conn)
            .optional()?
            .ok_or(api::Error::NotFound(ResourceType::TagCategory))?;
        TagCategoryInfo::new(conn, category, &fields).map_err(api::Error::from)
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NewTagCategoryInfo {
    order: i32,
    name: String,
    color: String,
}

fn create(auth: AuthResult, query: ResourceQuery, category_info: NewTagCategoryInfo) -> ApiResult<TagCategoryInfo> {
    let client = auth?;
    api::verify_privilege(client, config::privileges().tag_category_create)?;
    api::verify_matches_regex(&category_info.name, RegexType::TagCategory)?;

    let new_category = NewTagCategory {
        order: category_info.order,
        name: &category_info.name,
        color: &category_info.color,
    };

    let fields = resource::create_table(query.fields()).map_err(Box::from)?;
    let mut conn = db::get_connection()?;
    let category = diesel::insert_into(tag_category::table)
        .values(new_category)
        .returning(TagCategory::as_returning())
        .get_result(&mut conn)?;
    conn.transaction(|conn| TagCategoryInfo::new(conn, category, &fields).map_err(api::Error::from))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TagCategoryUpdate {
    version: DateTime,
    order: Option<String>, // TODO: Client sends order out as string so we convert on server, would be better to do this on client
    name: Option<String>,
    color: Option<String>,
}

fn update(
    auth: AuthResult,
    name: String,
    query: ResourceQuery,
    update: TagCategoryUpdate,
) -> ApiResult<TagCategoryInfo> {
    let client = auth?;
    let fields = resource::create_table(query.fields()).map_err(Box::from)?;
    let name = percent_encoding::percent_decode_str(&name).decode_utf8()?;

    let mut conn = db::get_connection()?;
    let category_id = conn.transaction(|conn| {
        let (category_id, last_edit_time) = tag_category::table
            .select((tag_category::id, tag_category::last_edit_time))
            .filter(tag_category::name.eq(name))
            .first(conn)?;
        api::verify_version(last_edit_time, update.version)?;

        if let Some(order) = update.order {
            api::verify_privilege(client, config::privileges().tag_category_edit_order)?;

            let order: i32 = order.parse()?;
            diesel::update(tag_category::table.find(category_id))
                .set(tag_category::order.eq(order))
                .execute(conn)?;
        }
        if let Some(name) = update.name {
            api::verify_privilege(client, config::privileges().tag_category_edit_name)?;
            api::verify_matches_regex(&name, RegexType::TagCategory)?;

            diesel::update(tag_category::table.find(category_id))
                .set(tag_category::name.eq(name))
                .execute(conn)?;
        }
        if let Some(color) = update.color {
            api::verify_privilege(client, config::privileges().tag_category_edit_color)?;

            diesel::update(tag_category::table.find(category_id))
                .set(tag_category::color.eq(color))
                .execute(conn)?;
        }

        // Update last_edit_time
        diesel::update(tag_category::table.find(category_id))
            .set(tag_category::last_edit_time.eq(DateTime::now()))
            .execute(conn)?;
        Ok::<_, api::Error>(category_id)
    })?;
    conn.transaction(|conn| TagCategoryInfo::new_from_id(conn, category_id, &fields).map_err(api::Error::from))
}

fn set_default(auth: AuthResult, name: String, query: ResourceQuery) -> ApiResult<TagCategoryInfo> {
    let client = auth?;
    api::verify_privilege(client, config::privileges().tag_category_set_default)?;

    let fields = resource::create_table(query.fields()).map_err(Box::from)?;
    let name = percent_encoding::percent_decode_str(&name).decode_utf8()?;
    let mut conn = db::get_connection()?;
    let new_default_category: TagCategory = conn.transaction(|conn| {
        let mut category: TagCategory = tag_category::table.filter(tag_category::name.eq(name)).first(conn)?;
        let mut old_default_category: TagCategory = tag_category::table.filter(tag_category::id.eq(0)).first(conn)?;

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
        let mut temporary_category_name = String::from("");
        std::mem::swap(&mut category.name, &mut temporary_category_name);
        let mut new_default_category: TagCategory = category.save_changes(conn)?;

        // Update what used to be default category
        let _: TagCategory = old_default_category.save_changes(conn)?;

        // Give new default category back it's name
        new_default_category.name = temporary_category_name;
        new_default_category.save_changes(conn)
    })?;
    conn.transaction(|conn| TagCategoryInfo::new(conn, new_default_category, &fields).map_err(api::Error::from))
}

fn delete(auth: AuthResult, name: String, client_version: DeleteRequest) -> ApiResult<()> {
    let client = auth?;
    api::verify_privilege(client, config::privileges().tag_category_delete)?;

    let name = percent_encoding::percent_decode_str(&name).decode_utf8()?;
    db::get_connection()?.transaction(|conn| {
        let (category_id, category_version): (i64, DateTime) = tag_category::table
            .select((tag_category::id, tag_category::last_edit_time))
            .filter(tag_category::name.eq(name))
            .first(conn)?;
        api::verify_version(category_version, *client_version)?;
        if category_id == 0 {
            return Err(api::Error::DeleteDefault(ResourceType::TagCategory));
        }

        diesel::delete(tag_category::table.find(category_id)).execute(conn)?;
        Ok(())
    })
}

#[cfg(test)]
mod test {
    use crate::api::ApiResult;
    use crate::model::tag::TagCategory;
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

        verify_query(&format!("DELETE /tag-category/{category_name}"), "delete.json").await?;

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
