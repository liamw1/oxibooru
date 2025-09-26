use crate::api::{ApiResult, PageParams, PagedResponse};
use crate::auth::Client;
use crate::resource::snapshot::SnapshotInfo;
use crate::search::snapshot::QueryBuilder;
use crate::{api, config, db, resource};
use axum::extract::Query;
use axum::{Extension, Json, Router, routing};
use diesel::prelude::*;

pub fn routes() -> Router {
    Router::new().route("/snapshots", routing::get(list))
}

const MAX_SNAPSHOTS_PER_PAGE: i64 = 1000;

async fn list(
    Extension(client): Extension<Client>,
    Query(params): Query<PageParams>,
) -> ApiResult<Json<PagedResponse<SnapshotInfo>>> {
    api::verify_privilege(client, config::privileges().snapshot_list)?;

    let offset = params.offset.unwrap_or(0);
    let limit = std::cmp::min(params.limit.get(), MAX_SNAPSHOTS_PER_PAGE);
    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    db::get_connection()?.transaction(|conn| {
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
