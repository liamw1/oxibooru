use crate::api::{ApiResult, PageParams, PagedResponse};
use crate::app::AppState;
use crate::auth::Client;
use crate::resource::snapshot::SnapshotInfo;
use crate::search::Builder;
use crate::search::snapshot::QueryBuilder;
use crate::{api, config, resource};
use axum::extract::{Query, State};
use axum::{Extension, Json, Router, routing};
use diesel::Connection;

pub fn routes() -> Router<AppState> {
    Router::new().route("/snapshots", routing::get(list))
}

const MAX_SNAPSHOTS_PER_PAGE: i64 = 1000;

/// See [listing-snapshots](https://github.com/liamw1/oxibooru/blob/master/doc/API.md#listing-snapshots)
async fn list(
    State(state): State<AppState>,
    Extension(client): Extension<Client>,
    Query(params): Query<PageParams>,
) -> ApiResult<Json<PagedResponse<SnapshotInfo>>> {
    api::verify_privilege(client, config::privileges().snapshot_list)?;

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
            results: SnapshotInfo::new_batch_from_ids(conn, &selected_snapshots, &fields)?,
        }))
    })
}
