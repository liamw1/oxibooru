use crate::api::{ApiResult, AuthResult, DeleteRequest, ResourceQuery, UnpagedResponse};
use crate::config::RegexType;
use crate::model::enums::ResourceType;
use crate::model::pool::{NewPoolCategory, PoolCategory};
use crate::resource::pool_category::{FieldTable, PoolCategoryInfo};
use crate::schema::{pool, pool_category};
use crate::time::DateTime;
use crate::{api, config, db, resource};
use diesel::prelude::*;
use serde::Deserialize;
use warp::{Filter, Rejection, Reply};

pub fn routes() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    let list = warp::get()
        .and(api::auth())
        .and(warp::path!("pool-categories"))
        .and(api::resource_query())
        .map(list)
        .map(api::Reply::from);
    let get = warp::get()
        .and(api::auth())
        .and(warp::path!("pool-category" / String))
        .and(api::resource_query())
        .map(get)
        .map(api::Reply::from);
    let create = warp::post()
        .and(api::auth())
        .and(warp::path!("pool-categories"))
        .and(api::resource_query())
        .and(warp::body::json())
        .map(create)
        .map(api::Reply::from);
    let update = warp::put()
        .and(api::auth())
        .and(warp::path!("pool-category" / String))
        .and(api::resource_query())
        .and(warp::body::json())
        .map(update)
        .map(api::Reply::from);
    let set_default = warp::put()
        .and(api::auth())
        .and(warp::path!("pool-category" / String / "default"))
        .and(api::resource_query())
        .map(set_default)
        .map(api::Reply::from);
    let delete = warp::delete()
        .and(api::auth())
        .and(warp::path!("pool-category" / String))
        .and(warp::body::json())
        .map(delete)
        .map(api::Reply::from);

    list.or(get).or(create).or(update).or(set_default).or(delete)
}

fn create_field_table(fields: Option<&str>) -> Result<FieldTable<bool>, Box<dyn std::error::Error>> {
    fields
        .map(resource::pool_category::Field::create_table)
        .transpose()
        .map(|opt_table| opt_table.unwrap_or(FieldTable::filled(true)))
        .map_err(Box::from)
}

fn list(auth: AuthResult, query: ResourceQuery) -> ApiResult<UnpagedResponse<PoolCategoryInfo>> {
    let client = auth?;
    api::verify_privilege(client, config::privileges().pool_category_list)?;

    let fields = create_field_table(query.fields())?;
    db::get_connection()?.transaction(|conn| {
        PoolCategoryInfo::all(conn, &fields)
            .map(|results| UnpagedResponse { results })
            .map_err(api::Error::from)
    })
}

fn get(auth: AuthResult, name: String, query: ResourceQuery) -> ApiResult<PoolCategoryInfo> {
    let client = auth?;
    api::verify_privilege(client, config::privileges().pool_category_view)?;

    let fields = create_field_table(query.fields())?;
    let name = percent_encoding::percent_decode_str(&name).decode_utf8()?;
    db::get_connection()?.transaction(|conn| {
        let category = pool_category::table
            .filter(pool_category::name.eq(name))
            .first(conn)
            .optional()?
            .ok_or(api::Error::NotFound(ResourceType::PoolCategory))?;
        PoolCategoryInfo::new(conn, category, &fields).map_err(api::Error::from)
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NewPoolCategoryInfo {
    name: String,
    color: String,
}

fn create(auth: AuthResult, query: ResourceQuery, category_info: NewPoolCategoryInfo) -> ApiResult<PoolCategoryInfo> {
    let client = auth?;
    api::verify_privilege(client, config::privileges().pool_category_create)?;
    api::verify_matches_regex(&category_info.name, RegexType::PoolCategory)?;

    let new_category = NewPoolCategory {
        name: &category_info.name,
        color: &category_info.color,
    };

    let fields = create_field_table(query.fields())?;
    let mut conn = db::get_connection()?;
    let category = diesel::insert_into(pool_category::table)
        .values(new_category)
        .returning(PoolCategory::as_returning())
        .get_result(&mut conn)?;
    conn.transaction(|conn| PoolCategoryInfo::new(conn, category, &fields).map_err(api::Error::from))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PoolCategoryUpdate {
    version: DateTime,
    name: Option<String>,
    color: Option<String>,
}

fn update(
    auth: AuthResult,
    name: String,
    query: ResourceQuery,
    update: PoolCategoryUpdate,
) -> ApiResult<PoolCategoryInfo> {
    let client = auth?;
    let fields = create_field_table(query.fields())?;
    let name = percent_encoding::percent_decode_str(&name).decode_utf8()?;

    let mut conn = db::get_connection()?;
    let category_id = conn.transaction(|conn| {
        let (category_id, last_edit_time) = pool_category::table
            .select((pool_category::id, pool_category::last_edit_time))
            .filter(pool_category::name.eq(name))
            .first(conn)?;
        api::verify_version(last_edit_time, update.version)?;

        let current_time = DateTime::now();
        if let Some(name) = update.name {
            api::verify_privilege(client, config::privileges().pool_category_edit_name)?;
            api::verify_matches_regex(&name, RegexType::PoolCategory)?;

            diesel::update(pool_category::table.find(category_id))
                .set((pool_category::name.eq(name), pool_category::last_edit_time.eq(current_time)))
                .execute(conn)?;
        }
        if let Some(color) = update.color {
            api::verify_privilege(client, config::privileges().pool_category_edit_color)?;

            diesel::update(pool_category::table.find(category_id))
                .set((pool_category::color.eq(color), pool_category::last_edit_time.eq(current_time)))
                .execute(conn)?;
        }
        Ok::<_, api::Error>(category_id)
    })?;
    conn.transaction(|conn| PoolCategoryInfo::new_from_id(conn, category_id, &fields).map_err(api::Error::from))
}

fn set_default(auth: AuthResult, name: String, query: ResourceQuery) -> ApiResult<PoolCategoryInfo> {
    let client = auth?;
    api::verify_privilege(client, config::privileges().pool_category_set_default)?;

    let fields = create_field_table(query.fields())?;
    let name = percent_encoding::percent_decode_str(&name).decode_utf8()?;
    let mut conn = db::get_connection()?;
    let new_default_category: PoolCategory = conn.transaction(|conn| {
        let mut category: PoolCategory = pool_category::table.filter(pool_category::name.eq(name)).first(conn)?;
        let mut old_default_category: PoolCategory =
            pool_category::table.filter(pool_category::id.eq(0)).first(conn)?;

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
        let mut temporary_category_name = String::from("");
        std::mem::swap(&mut category.name, &mut temporary_category_name);
        let mut new_default_category: PoolCategory = category.save_changes(conn)?;

        // Update what used to be default category
        let _: PoolCategory = old_default_category.save_changes(conn)?;

        // Give new default category back it's name
        new_default_category.name = temporary_category_name;
        new_default_category.save_changes(conn)
    })?;
    conn.transaction(|conn| PoolCategoryInfo::new(conn, new_default_category, &fields).map_err(api::Error::from))
}

fn delete(auth: AuthResult, name: String, client_version: DeleteRequest) -> ApiResult<()> {
    let client = auth?;
    api::verify_privilege(client, config::privileges().pool_category_delete)?;

    let name = percent_encoding::percent_decode_str(&name).decode_utf8()?;
    db::get_connection()?.transaction(|conn| {
        let (category_id, category_version): (i64, DateTime) = pool_category::table
            .select((pool_category::id, pool_category::last_edit_time))
            .filter(pool_category::name.eq(name))
            .first(conn)?;
        api::verify_version(category_version, *client_version)?;
        if category_id == 0 {
            return Err(api::Error::DeleteDefault(ResourceType::PoolCategory));
        }

        diesel::delete(pool_category::table.find(category_id)).execute(conn)?;
        Ok(())
    })
}

#[cfg(test)]
mod test {
    use crate::api::ApiResult;
    use crate::model::pool::PoolCategory;
    use crate::schema::{pool_category, pool_category_statistics};
    use crate::test::*;
    use crate::time::DateTime;
    use diesel::prelude::*;
    use serial_test::{parallel, serial};

    // Exclude fields that involve creation_time or last_edit_time
    const FIELDS: &str = "&fields=name,color,usages,default";

    #[tokio::test]
    #[parallel]
    async fn list() -> ApiResult<()> {
        verify_query(&format!("GET /pool-categories/?{FIELDS}"), "pool_category/list.json").await
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

        verify_query(&format!("GET /pool-category/{NAME}/?{FIELDS}"), "pool_category/get.json").await?;

        let new_last_edit_time = get_last_edit_time(&mut conn)?;
        assert_eq!(new_last_edit_time, last_edit_time);
        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn create() -> ApiResult<()> {
        let mut conn = get_connection()?;
        let category_count: i64 = pool_category::table.count().first(&mut conn)?;

        verify_query(&format!("POST /pool-categories/?{FIELDS}"), "pool_category/create.json").await?;

        let category_name: String = pool_category::table
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

        verify_query(&format!("DELETE /pool-category/{category_name}"), "delete.json").await?;

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

        verify_query(&format!("PUT /pool-category/{NAME}/?{FIELDS}"), "pool_category/update.json").await?;

        let updated_category: PoolCategory = pool_category::table
            .filter(pool_category::id.eq(category.id))
            .first(&mut conn)?;
        assert_ne!(updated_category.name, category.name);
        assert_ne!(updated_category.color, category.color);
        assert!(updated_category.last_edit_time > category.last_edit_time);

        let new_name = updated_category.name;
        verify_query(&format!("PUT /pool-category/{new_name}/?{FIELDS}"), "pool_category/update_restore.json").await
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

        verify_query(&format!("PUT /pool-category/{NAME}/default/?{FIELDS}"), "pool_category/set_default.json").await?;

        let mut conn = get_connection()?;
        let default = is_default(&mut conn)?;
        assert!(default);

        verify_query(&format!("PUT /pool-category/default/default/?{FIELDS}"), "pool_category/restore_default.json")
            .await?;

        let default = is_default(&mut conn)?;
        assert!(!default);
        Ok(())
    }
}
