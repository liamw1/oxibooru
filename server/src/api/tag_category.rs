use crate::api::{ApiResult, AuthResult, ResourceVersion};
use crate::model::tag::{NewTagCategory, TagCategory};
use crate::resource::tag_category::TagCategoryInfo;
use crate::schema::{tag, tag_category};
use crate::util::DateTime;
use crate::{api, config};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use warp::{Filter, Rejection, Reply};

pub fn routes() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    let list_tag_categories = warp::get()
        .and(warp::path!("tag-categories"))
        .and(api::auth())
        .map(list_tag_categories)
        .map(api::Reply::from);
    let get_tag_category = warp::get()
        .and(warp::path!("tag-category" / String))
        .and(api::auth())
        .map(get_tag_category)
        .map(api::Reply::from);
    let create_tag_category = warp::post()
        .and(warp::path!("tag-categories"))
        .and(api::auth())
        .and(warp::body::json())
        .map(create_tag_category)
        .map(api::Reply::from);
    let update_tag_category = warp::put()
        .and(warp::path!("tag-category" / String))
        .and(api::auth())
        .and(warp::body::json())
        .map(update_tag_category)
        .map(api::Reply::from);
    let set_default_category = warp::put()
        .and(warp::path!("tag-category" / String / "default"))
        .and(api::auth())
        .map(set_default_category)
        .map(api::Reply::from);
    let delete_tag_category = warp::delete()
        .and(warp::path!("tag-category" / String))
        .and(api::auth())
        .and(warp::body::json())
        .map(delete_tag_category)
        .map(api::Reply::from);

    list_tag_categories
        .or(get_tag_category)
        .or(create_tag_category)
        .or(update_tag_category)
        .or(set_default_category)
        .or(delete_tag_category)
}

#[derive(Serialize)]
struct TagCategoryList {
    results: Vec<TagCategoryInfo>,
}

fn list_tag_categories(auth: AuthResult) -> ApiResult<TagCategoryList> {
    let client = auth?;
    api::verify_privilege(client.as_ref(), config::privileges().tag_category_list)?;

    crate::establish_connection()?.transaction(|conn| {
        TagCategoryInfo::all(conn)
            .map(|results| TagCategoryList { results })
            .map_err(api::Error::from)
    })
}

fn get_tag_category(name: String, auth: AuthResult) -> ApiResult<TagCategoryInfo> {
    let client = auth?;
    api::verify_privilege(client.as_ref(), config::privileges().tag_category_view)?;

    let name = percent_encoding::percent_decode_str(&name).decode_utf8()?;
    crate::establish_connection()?.transaction(|conn| {
        let category = tag_category::table.filter(tag_category::name.eq(name)).first(conn)?;
        TagCategoryInfo::new(conn, category).map_err(api::Error::from)
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NewTagCategoryInfo {
    order: i32,
    name: String,
    color: String,
}

fn create_tag_category(auth: AuthResult, category_info: NewTagCategoryInfo) -> ApiResult<TagCategoryInfo> {
    let client = auth?;
    api::verify_privilege(client.as_ref(), config::privileges().tag_category_create)?;

    crate::establish_connection()?.transaction(|conn| {
        let new_category = NewTagCategory {
            order: category_info.order,
            name: &category_info.name,
            color: &category_info.color,
        };
        let category = diesel::insert_into(tag_category::table)
            .values(new_category)
            .returning(TagCategory::as_returning())
            .get_result(conn)?;

        TagCategoryInfo::new(conn, category).map_err(api::Error::from)
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TagCategoryUpdateInfo {
    version: DateTime,
    name: Option<String>,
    color: Option<String>,
    order: Option<String>, // TODO: Client sends order out as string so we convert on server, would be better to do this on client
}

fn update_tag_category(name: String, auth: AuthResult, update: TagCategoryUpdateInfo) -> ApiResult<TagCategoryInfo> {
    let client = auth?;
    let name = percent_encoding::percent_decode_str(&name).decode_utf8()?;
    crate::establish_connection()?.transaction(|conn| {
        let category = TagCategory::from_name(conn, &name)?;
        api::verify_version(category.last_edit_time, update.version)?;

        if let Some(name) = update.name {
            api::verify_privilege(client.as_ref(), config::privileges().tag_category_edit_name)?;

            diesel::update(tag_category::table.find(category.id))
                .set(tag_category::name.eq(name))
                .execute(conn)?;
        }

        if let Some(color) = update.color {
            api::verify_privilege(client.as_ref(), config::privileges().tag_category_edit_color)?;

            diesel::update(tag_category::table.find(category.id))
                .set(tag_category::color.eq(color))
                .execute(conn)?;
        }

        if let Some(order) = update.order {
            api::verify_privilege(client.as_ref(), config::privileges().tag_category_edit_order)?;

            let order: i32 = order.parse()?;
            diesel::update(tag_category::table.find(category.id))
                .set(tag_category::order.eq(order))
                .execute(conn)?;
        }

        TagCategoryInfo::new_from_id(conn, category.id).map_err(api::Error::from)
    })
}

fn set_default_category(name: String, auth: AuthResult) -> ApiResult<TagCategoryInfo> {
    let _timer = crate::util::Timer::new("set_default_category");

    let client = auth?;
    api::verify_privilege(client.as_ref(), config::privileges().tag_category_set_default)?;

    let name = percent_encoding::percent_decode_str(&name).decode_utf8()?;
    crate::establish_connection()?.transaction(|conn| {
        let mut category: TagCategory = tag_category::table.filter(tag_category::name.eq(name)).first(conn)?;
        let mut old_default_category: TagCategory = tag_category::table.filter(tag_category::id.eq(0)).first(conn)?;

        let defaulted_tags: Vec<i32> = diesel::update(tag::table)
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
        let mut temporary_category_name = String::from("");
        std::mem::swap(&mut category.name, &mut temporary_category_name);
        let mut new_default_category: TagCategory = category.save_changes(conn)?;

        // Update what used to be default category
        let _: TagCategory = old_default_category.save_changes(conn)?;

        // Give new default category back it's name
        new_default_category.name = temporary_category_name;
        let new_default_category: TagCategory = new_default_category.save_changes(conn)?;

        TagCategoryInfo::new(conn, new_default_category).map_err(api::Error::from)
    })
}

fn delete_tag_category(name: String, auth: AuthResult, client_version: ResourceVersion) -> ApiResult<()> {
    let client = auth?;
    api::verify_privilege(client.as_ref(), config::privileges().tag_category_delete)?;

    let name = percent_encoding::percent_decode_str(&name).decode_utf8()?;
    crate::establish_connection()?.transaction(|conn| {
        let (category_id, category_version): (i32, DateTime) = tag_category::table
            .select((tag_category::id, tag_category::last_edit_time))
            .filter(tag_category::name.eq(name))
            .first(conn)?;
        api::verify_version(category_version, *client_version)?;
        if category_id == 0 {
            return Err(api::Error::InvalidRequest);
        }

        diesel::delete(tag_category::table.find(category_id)).execute(conn)?;
        Ok(())
    })
}
