use crate::api::doc::SNAPSHOT_TAG;
use crate::api::error::{ApiError, ApiResult};
use crate::api::extract::{Json, Query};
use crate::api::{PageParams, PagedResponse, ResourceParams};
use crate::app::AppState;
use crate::auth::Client;
use crate::resource::snapshot::SnapshotInfo;
use crate::search::Builder;
use crate::search::snapshot::QueryBuilder;
use crate::{api, resource};
use axum::extract::{Extension, State};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(routes!(list))
}

const MAX_SNAPSHOTS_PER_PAGE: i64 = 1000;

/// Lists recent resource snapshots.
///
/// **Anonymous tokens**
///
/// Not supported.
///
/// **Named tokens**
///
/// | Key            | Description                                                      |
/// | -------------- | ---------------------------------------------------------------- |
/// | `type`         | involving given resource type                                    |
/// | `id`           | involving given resource id                                      |
/// | `date`, `time` | created at given date                                            |
/// | `operation`    | `modified`, `created`, `deleted` or `merged`                     |
/// | `user`         | name of the user that created given snapshot (accepts wildcards) |
///
/// **Sort style tokens**
///
/// None. The snapshots are always sorted by creation time.
///
/// **Special tokens**
///
/// None.
#[utoipa::path(
    get,
    path = "/snapshots",
    tag = SNAPSHOT_TAG,
    params(ResourceParams, PageParams),
    responses(
        (status = 200, body = PagedResponse<SnapshotInfo>),
        (status = 403, description = "Privileges are too low"),
    ),
)]
async fn list(
    State(state): State<AppState>,
    Extension(client): Extension<Client>,
    Query(resource): Query<ResourceParams>,
    Query(page): Query<PageParams>,
) -> ApiResult<Json<PagedResponse<SnapshotInfo>>> {
    api::verify_privilege(client, state.config.privileges().snapshot_list)?;

    let offset = page.offset.unwrap_or(0);
    let limit = std::cmp::min(page.limit.get(), MAX_SNAPSHOTS_PER_PAGE);
    let fields = resource::create_table(resource.fields()).map_err(Box::from)?;
    state
        .connection_pool
        .transaction(move |conn| {
            let mut query_builder = QueryBuilder::new(client, resource.criteria())?;
            query_builder.set_offset_and_limit(offset, limit);

            let (total, selected_snapshots) = query_builder.list(conn)?;
            Ok::<_, ApiError>(Json(PagedResponse {
                query: resource.query,
                offset,
                limit,
                total,
                results: SnapshotInfo::new_batch_from_ids(conn, &state.config, &selected_snapshots, &fields)?,
            }))
        })
        .await
}
