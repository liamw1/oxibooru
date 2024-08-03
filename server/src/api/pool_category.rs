use crate::api::{ApiResult, AuthResult, DeleteRequest};
use crate::config::RegexType;
use crate::model::pool::{NewPoolCategory, PoolCategory};
use crate::resource::pool_category::PoolCategoryInfo;
use crate::schema::{pool, pool_category};
use crate::util::DateTime;
use crate::{api, config};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use warp::{Filter, Rejection, Reply};

pub fn routes() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    let list_pool_categories = warp::get()
        .and(warp::path!("pool-categories"))
        .and(api::auth())
        .map(list_pool_categories)
        .map(api::Reply::from);
    let get_pool_category = warp::get()
        .and(warp::path!("pool-category" / String))
        .and(api::auth())
        .map(get_pool_category)
        .map(api::Reply::from);
    let create_pool_category = warp::post()
        .and(warp::path!("pool-categories"))
        .and(api::auth())
        .and(warp::body::json())
        .map(create_pool_category)
        .map(api::Reply::from);
    let update_pool_category = warp::put()
        .and(warp::path!("pool-category" / String))
        .and(api::auth())
        .and(warp::body::json())
        .map(update_pool_category)
        .map(api::Reply::from);
    let set_default_pool_category = warp::put()
        .and(warp::path!("pool-category" / String / "default"))
        .and(api::auth())
        .map(set_default_pool_category)
        .map(api::Reply::from);
    let delete_pool_category = warp::delete()
        .and(warp::path!("pool-category" / String))
        .and(api::auth())
        .and(warp::body::json())
        .map(delete_pool_category)
        .map(api::Reply::from);

    list_pool_categories
        .or(get_pool_category)
        .or(create_pool_category)
        .or(update_pool_category)
        .or(set_default_pool_category)
        .or(delete_pool_category)
}

#[derive(Serialize)]
struct PoolCategoryList {
    results: Vec<PoolCategoryInfo>,
}

fn list_pool_categories(auth: AuthResult) -> ApiResult<PoolCategoryList> {
    let client = auth?;
    api::verify_privilege(client.as_ref(), config::privileges().pool_category_list)?;

    crate::establish_connection()?.transaction(|conn| {
        PoolCategoryInfo::all(conn)
            .map(|results| PoolCategoryList { results })
            .map_err(api::Error::from)
    })
}

fn get_pool_category(name: String, auth: AuthResult) -> ApiResult<PoolCategoryInfo> {
    let client = auth?;
    api::verify_privilege(client.as_ref(), config::privileges().pool_category_view)?;

    let name = percent_encoding::percent_decode_str(&name).decode_utf8()?;
    crate::establish_connection()?.transaction(|conn| {
        let category = pool_category::table.filter(pool_category::name.eq(name)).first(conn)?;
        PoolCategoryInfo::new(conn, category).map_err(api::Error::from)
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NewPoolCategoryInfo {
    name: String,
    color: String,
}

fn create_pool_category(auth: AuthResult, category_info: NewPoolCategoryInfo) -> ApiResult<PoolCategoryInfo> {
    let client = auth?;
    api::verify_privilege(client.as_ref(), config::privileges().pool_category_create)?;
    api::verify_matches_regex(&category_info.name, RegexType::PoolCategory)?;

    crate::establish_connection()?.transaction(|conn| {
        let new_category = NewPoolCategory {
            name: &category_info.name,
            color: &category_info.color,
        };
        let category = diesel::insert_into(pool_category::table)
            .values(new_category)
            .returning(PoolCategory::as_returning())
            .get_result(conn)?;

        PoolCategoryInfo::new(conn, category).map_err(api::Error::from)
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PoolCategoryUpdateInfo {
    version: DateTime,
    name: Option<String>,
    color: Option<String>,
}

fn update_pool_category(name: String, auth: AuthResult, update: PoolCategoryUpdateInfo) -> ApiResult<PoolCategoryInfo> {
    let client = auth?;
    let name = percent_encoding::percent_decode_str(&name).decode_utf8()?;
    api::verify_matches_regex(&name, RegexType::PoolCategory)?;

    crate::establish_connection()?.transaction(|conn| {
        let category = PoolCategory::from_name(conn, &name)?;
        api::verify_version(category.last_edit_time, update.version)?;

        if let Some(name) = update.name {
            api::verify_privilege(client.as_ref(), config::privileges().pool_category_edit_name)?;

            diesel::update(pool_category::table.find(category.id))
                .set(pool_category::name.eq(name))
                .execute(conn)?;
        }

        if let Some(color) = update.color {
            api::verify_privilege(client.as_ref(), config::privileges().pool_category_edit_color)?;

            diesel::update(pool_category::table.find(category.id))
                .set(pool_category::color.eq(color))
                .execute(conn)?;
        }

        PoolCategoryInfo::new_from_id(conn, category.id).map_err(api::Error::from)
    })
}

fn set_default_pool_category(name: String, auth: AuthResult) -> ApiResult<PoolCategoryInfo> {
    let _timer = crate::util::Timer::new("set_default_pool_category");

    let client = auth?;
    api::verify_privilege(client.as_ref(), config::privileges().pool_category_set_default)?;

    let name = percent_encoding::percent_decode_str(&name).decode_utf8()?;
    crate::establish_connection()?.transaction(|conn| {
        let mut category: PoolCategory = pool_category::table.filter(pool_category::name.eq(name)).first(conn)?;
        let mut old_default_category: PoolCategory =
            pool_category::table.filter(pool_category::id.eq(0)).first(conn)?;

        let defaulted_pools: Vec<i32> = diesel::update(pool::table)
            .filter(pool::category_id.eq(category.id))
            .set(pool::category_id.eq(0))
            .returning(pool::id)
            .get_results(conn)?;
        diesel::update(pool::table)
            .filter(pool::category_id.eq(0))
            .filter(pool::id.ne_all(defaulted_pools))
            .set(pool::category_id.eq(category.id))
            .execute(conn)?;

        // Make category default
        std::mem::swap(&mut category.id, &mut old_default_category.id);

        // Give new default category an empty name so it doesn't violate uniqueness
        let mut temporary_category_name = String::from("");
        std::mem::swap(&mut category.name, &mut temporary_category_name);
        let mut new_default_category: PoolCategory = category.save_changes(conn)?;

        // Update what used to be default category
        let _: PoolCategory = old_default_category.save_changes(conn)?;

        // Give new default category back it's name
        new_default_category.name = temporary_category_name;
        let new_default_category: PoolCategory = new_default_category.save_changes(conn)?;

        PoolCategoryInfo::new(conn, new_default_category).map_err(api::Error::from)
    })
}

fn delete_pool_category(name: String, auth: AuthResult, client_version: DeleteRequest) -> ApiResult<()> {
    let client = auth?;
    api::verify_privilege(client.as_ref(), config::privileges().pool_category_delete)?;

    let name = percent_encoding::percent_decode_str(&name).decode_utf8()?;
    crate::establish_connection()?.transaction(|conn| {
        let (category_id, category_version): (i32, DateTime) = pool_category::table
            .select((pool_category::id, pool_category::last_edit_time))
            .filter(pool_category::name.eq(name))
            .first(conn)?;
        api::verify_version(category_version, *client_version)?;
        if category_id == 0 {
            return Err(api::Error::DeleteDefault);
        }

        diesel::delete(pool_category::table.find(category_id)).execute(conn)?;
        Ok(())
    })
}
