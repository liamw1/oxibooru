use crate::api::doc::UPLOAD_TAG;
use crate::api::error::{ApiError, ApiResult};
use crate::app::AppState;
use crate::config::Action;
use crate::content::upload::{MAX_UPLOAD_SIZE, PartName, UploadToken};
use crate::content::{download, upload};
use crate::extract::{Ctx, Json, JsonOrMultipart};
use axum::extract::DefaultBodyLimit;
use serde::{Deserialize, Serialize};
use url::Url;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(upload))
        .route_layer(DefaultBodyLimit::max(MAX_UPLOAD_SIZE))
}

/// Request body for uploading a temporary file.
#[derive(Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct UploadBody {
    /// URL to fetch content from.
    content_url: Url,
}

/// Multipart form for file uploads.
#[allow(dead_code)]
#[derive(ToSchema)]
struct MultipartUpload {
    /// JSON metadata (same structure as JSON request body).
    metadata: UploadBody,
    /// Content file (image, video, etc.).
    #[schema(format = Binary)]
    content: Option<String>,
}

/// Response containing the upload token.
#[derive(Serialize, ToSchema)]
struct UploadResponse {
    /// Token to reference this upload in other requests.
    #[schema(value_type = String)]
    token: UploadToken,
}

async fn upload_from_url(ctx: &Ctx, body: UploadBody) -> ApiResult<Json<UploadResponse>> {
    let token = download::from_url(ctx, body.content_url).await?;
    Ok(Json(UploadResponse { token }))
}

/// Puts a file in temporary storage and assigns it a token.
///
/// The token can be used in other requests. Files uploaded this way are
/// deleted after a short while so clients shouldn't use it as a free upload
/// service. Note that in this particular API, one can't use token-based uploads.
#[utoipa::path(
    post,
    path = "/uploads",
    tag = UPLOAD_TAG,
    request_body(
        content(
            (UploadBody = "application/json"),
            (MultipartUpload = "multipart/form-data"),
        )
    ),
    responses(
        (status = 200, body = UploadResponse),
        (status = 403, description = "Privileges are too low"),
    ),
)]
async fn upload(ctx: Ctx, body: JsonOrMultipart<UploadBody>) -> ApiResult<Json<UploadResponse>> {
    ctx.verify_privilege(Action::UploadCreate)?;

    match body {
        JsonOrMultipart::Json(payload) => upload_from_url(&ctx, payload).await,
        JsonOrMultipart::Multipart(payload) => {
            let decoded_body = upload::extract(&ctx.config, payload, [PartName::Content]).await?;
            if let [Some(token)] = decoded_body.files {
                Ok(Json(UploadResponse { token }))
            } else if let Some(metadata) = decoded_body.metadata {
                let url_upload: UploadBody = serde_json::from_slice(&metadata)?;
                upload_from_url(&ctx, url_upload).await
            } else {
                Err(ApiError::MissingFormData)
            }
        }
    }
}

#[cfg(test)]
mod test {
    use crate::api::error::ApiResult;
    use crate::model::enums::UserRank;
    use crate::test::*;
    use serial_test::parallel;

    #[tokio::test]
    #[parallel]
    async fn unauthorized() -> ApiResult<()> {
        const USER: UserRank = UserRank::Regular;
        verify_response_with_user(USER, "POST /uploads", "upload/create_unauthorized").await?;
        verify_response_with_user(USER, "POST /uploads", "upload/download_unauthorized").await
    }
}
