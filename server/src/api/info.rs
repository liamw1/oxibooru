use crate::api::ResourceParams;
use crate::api::doc::INFO_TAG;
use crate::api::error::ApiResult;
use crate::api::extract::{Json, Query};
use crate::app::AppState;
use crate::auth::Client;
use crate::config::PublicConfig;
use crate::model::post::PostFeature;
use crate::resource;
use crate::resource::post::PostInfo;
use crate::schema::{database_statistics, post_feature, user};
use crate::string::SmallString;
use crate::time::DateTime;
use axum::extract::{Extension, State};
use diesel::{Connection, ExpressionMethods, OptionalExtension, QueryDsl, RunQueryDsl};
use serde::Serialize;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(routes!(get))
}

/// Server information response.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct InfoResponse {
    /// Total number of posts on the server.
    post_count: i64,
    /// Total disk usage in bytes.
    disk_usage: i64,
    /// The currently featured post, or null if none.
    featured_post: Option<PostInfo>,
    /// Time when the currently featured post was featured.
    featuring_time: Option<DateTime>,
    /// Username of the user who featured the currently featured post.
    featuring_user: Option<SmallString>,
    /// Current server time.
    server_time: DateTime,
    /// Public server configuration.
    config: PublicConfig,
}

/// Retrieves simple statistics.
///
/// `featuredPost` is null if there is no featured post yet. `serverTime` is
/// pretty much the same as the `Date` HTTP field, only formatted in a manner
/// consistent with other dates. Values in the `config` key are taken directly
/// from the server config.
#[utoipa::path(
    get, 
    path = "/info", 
    tag = INFO_TAG,
    params(ResourceParams),
    responses(
        (status = 200, description = "Server information", body = InfoResponse),
    ),
)]
async fn get(
    State(state): State<AppState>,
    Extension(client): Extension<Client>,
    Query(params): Query<ResourceParams>,
) -> ApiResult<Json<InfoResponse>> {
    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    state.get_connection()?.transaction(|conn| {
        let (post_count, disk_usage) = database_statistics::table
            .select((database_statistics::post_count, database_statistics::disk_usage))
            .first(conn)?;
        let latest_feature: Option<PostFeature> = post_feature::table
            .order_by(post_feature::time.desc())
            .first(conn)
            .optional()?;
        let featured_post: Option<PostInfo> = latest_feature
            .as_ref()
            .map(|feature| PostInfo::new_from_id(conn, &state.config, client, feature.post_id, &fields))
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

        Ok(Json(InfoResponse {
            post_count,
            disk_usage,
            featured_post,
            featuring_time: latest_feature.as_ref().map(|feature| feature.time),
            featuring_user,
            server_time: DateTime::now(),
            config: state.config.public_info.clone(),
        }))
    })
}
