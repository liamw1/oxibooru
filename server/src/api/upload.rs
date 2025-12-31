use crate::api;
use crate::api::doc::UPLOAD_TAG;
use crate::api::error::{ApiError, ApiResult};
use crate::api::extract::Json;
use crate::api::extract::JsonOrMultipart;
use crate::app::AppState;
use crate::auth::Client;
use crate::config::Config;
use crate::content::download;
use crate::content::upload::{self, MAX_UPLOAD_SIZE, PartName};
use axum::extract::{DefaultBodyLimit, Extension, State};
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
#[serde(deny_unknown_fields, rename_all = "camelCase")]
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
    token: String,
}

async fn upload_from_url(config: &Config, body: UploadBody) -> ApiResult<Json<UploadResponse>> {
    let token = download::from_url(config, body.content_url).await?;
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
async fn upload(
    State(state): State<AppState>,
    Extension(client): Extension<Client>,
    body: JsonOrMultipart<UploadBody>,
) -> ApiResult<Json<UploadResponse>> {
    api::verify_privilege(client, state.config.privileges().upload_create)?;

    match body {
        JsonOrMultipart::Json(payload) => upload_from_url(&state.config, payload).await,
        JsonOrMultipart::Multipart(payload) => {
            let decoded_body = upload::extract(payload, [PartName::Content]).await?;
            if let [Some(upload)] = decoded_body.files {
                let token = upload.save(&state.config)?;
                Ok(Json(UploadResponse { token }))
            } else if let Some(metadata) = decoded_body.metadata {
                let url_upload: UploadBody = serde_json::from_slice(&metadata)?;
                upload_from_url(&state.config, url_upload).await
            } else {
                Err(ApiError::MissingFormData)
            }
        }
    }
}
