use crate::api::doc::SNAPSHOT_TAG;
use crate::api::error::{ApiError, ApiResult};
use crate::api::{PageParams, PagedResponse, ResourceParams};
use crate::app::AppState;
use crate::config::Action;
use crate::extract::{Ctx, Json, Query};
use crate::resource::snapshot::{Field, SnapshotInfo};
use crate::search::Builder;
use crate::search::snapshot::QueryBuilder;
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
    Ctx(ctx, connection_pool): Ctx,
    Query(resource): Query<ResourceParams<Field>>,
    Query(page): Query<PageParams>,
) -> ApiResult<Json<PagedResponse<SnapshotInfo>>> {
    ctx.verify_privilege(Action::SnapshotList)?;

    let offset = page.offset.unwrap_or(0);
    let limit = std::cmp::min(page.limit.get(), MAX_SNAPSHOTS_PER_PAGE);
    connection_pool
        .transaction(move |conn| {
            let mut query_builder = QueryBuilder::new(&ctx, resource.criteria())?;
            query_builder.set_offset_and_limit(offset, limit);

            let (total, selected_snapshots) = query_builder.list(conn)?;
            Ok::<_, ApiError>(Json(PagedResponse {
                query: resource.query,
                offset,
                limit,
                total,
                results: SnapshotInfo::new_batch_from_ids(conn, &ctx.config, &selected_snapshots, resource.fields)?,
            }))
        })
        .await
}
