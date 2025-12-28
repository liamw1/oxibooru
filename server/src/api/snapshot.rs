use crate::api::error::ApiResult;
use crate::api::extract::{Json, Query};
use crate::api::{PageParams, PagedResponse, SNAPSHOT_TAG};
use crate::app::AppState;
use crate::auth::Client;
use crate::resource::snapshot::SnapshotInfo;
use crate::search::Builder;
use crate::search::snapshot::QueryBuilder;
use crate::{api, resource};
use axum::extract::{Extension, State};
use diesel::Connection;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(routes!(list))
}

const MAX_SNAPSHOTS_PER_PAGE: i64 = 1000;

#[utoipa::path(get, path = "/snapshots", tag = SNAPSHOT_TAG)]
async fn list(
    State(state): State<AppState>,
    Extension(client): Extension<Client>,
    Query(params): Query<PageParams>,
) -> ApiResult<Json<PagedResponse<SnapshotInfo>>> {
    api::verify_privilege(client, state.config.privileges().snapshot_list)?;

    let offset = params.offset.unwrap_or(0);
    let limit = std::cmp::min(params.limit.get(), MAX_SNAPSHOTS_PER_PAGE);
    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    state.get_connection()?.transaction(|conn| {
        let mut query_builder = QueryBuilder::new(client, params.criteria())?;
        query_builder.set_offset_and_limit(offset, limit);

        let (total, selected_snapshots) = query_builder.list(conn)?;
        Ok(Json(PagedResponse {
            query: params.into_query(),
            offset,
            limit,
            total,
            results: SnapshotInfo::new_batch_from_ids(conn, &state.config, &selected_snapshots, &fields)?,
        }))
    })
}
