use crate::api::{ApiResult, AuthResult, ResourceQuery};
use crate::model::post::PostFeature;
use crate::resource::post::{FieldTable, PostInfo};
use crate::schema::{database_statistics, post_feature, user};
use crate::time::DateTime;
use crate::{api, config, db, resource};
use diesel::prelude::*;
use serde::Serialize;
use warp::{Filter, Rejection, Reply};

pub fn routes() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    warp::get()
        .and(api::auth())
        .and(warp::path!("info"))
        .and(api::resource_query())
        .map(get_info)
        .map(api::Reply::from)
}

fn create_field_table(fields: Option<&str>) -> Result<FieldTable<bool>, Box<dyn std::error::Error>> {
    fields
        .map(resource::post::Field::create_table)
        .transpose()
        .map(|opt_table| opt_table.unwrap_or(FieldTable::filled(true)))
        .map_err(Box::from)
}

// TODO: Remove renames by changing references to these names in client
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Info {
    post_count: i64,
    disk_usage: i64,
    featured_post: Option<PostInfo>,
    featuring_time: Option<DateTime>,
    featuring_user: Option<String>,
    server_time: DateTime,
    config: &'static config::PublicInfo,
}

fn get_info(auth: AuthResult, query: ResourceQuery) -> ApiResult<Info> {
    let client = auth?;
    query.bump_login(client)?;

    let client_id = client.map(|user| user.id);
    let fields = create_field_table(query.fields())?;

    db::get_connection()?.transaction(|conn| {
        let (post_count, disk_usage) = database_statistics::table
            .select((database_statistics::post_count, database_statistics::disk_usage))
            .first(conn)?;
        let latest_feature: Option<PostFeature> = post_feature::table
            .order_by(post_feature::time.desc())
            .first(conn)
            .optional()?;
        let featured_post: Option<PostInfo> = latest_feature
            .as_ref()
            .map(|feature| PostInfo::new_from_id(conn, client_id, feature.post_id, &fields))
            .transpose()?;
        let featuring_user: Option<String> = latest_feature
            .as_ref()
            .map(|feature| {
                user::table
                    .find(feature.user_id)
                    .select(user::name)
                    .first(conn)
                    .optional()
            })
            .transpose()?
            .flatten();

        Ok(Info {
            post_count,
            disk_usage,
            featured_post,
            featuring_time: latest_feature.as_ref().map(|feature| feature.time),
            featuring_user,
            server_time: DateTime::now(),
            config: &config::get().public_info,
        })
    })
}
