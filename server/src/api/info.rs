use crate::api::{ApiResult, ResourceParams};
use crate::auth::Client;
use crate::config::PublicConfig;
use crate::model::post::PostFeature;
use crate::resource::post::PostInfo;
use crate::schema::{database_statistics, post_feature, user};
use crate::string::SmallString;
use crate::time::DateTime;
use crate::{config, db, resource};
use axum::extract::{Extension, Query};
use axum::response::Json;
use axum::routing::{self, Router};
use diesel::{Connection, ExpressionMethods, OptionalExtension, QueryDsl, RunQueryDsl};
use serde::Serialize;

pub fn routes() -> Router {
    Router::new().route("/info", routing::get(get))
}

// TODO: Remove renames by changing references to these names in client
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Response {
    post_count: i64,
    disk_usage: i64,
    featured_post: Option<PostInfo>,
    featuring_time: Option<DateTime>,
    featuring_user: Option<SmallString>,
    server_time: DateTime,
    config: &'static PublicConfig,
}

/// See [getting-global-info](https://github.com/liamw1/oxibooru/blob/master/doc/API.md#getting-global-info)
async fn get(Extension(client): Extension<Client>, Query(params): Query<ResourceParams>) -> ApiResult<Json<Response>> {
    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
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
            .map(|feature| PostInfo::new_from_id(conn, client, feature.post_id, &fields))
            .transpose()?;
        let featuring_user: Option<SmallString> = latest_feature
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

        Ok(Json(Response {
            post_count,
            disk_usage,
            featured_post,
            featuring_time: latest_feature.as_ref().map(|feature| feature.time),
            featuring_user,
            server_time: DateTime::now(),
            config: &config::get().public_info,
        }))
    })
}
