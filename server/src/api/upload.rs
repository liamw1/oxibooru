use crate::api::{ApiResult, AuthResult, MAX_UPLOAD_SIZE};
use crate::filesystem::Directory;
use crate::model::enums::MimeType;
use crate::{api, config, filesystem};
use futures::{StreamExt, TryStreamExt};
use serde::Serialize;
use std::str::FromStr;
use warp::multipart::FormData;
use warp::Buf;
use warp::{Filter, Rejection, Reply};

pub fn routes() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    warp::post()
        .and(warp::path!("uploads"))
        .and(api::auth())
        .and(warp::filters::multipart::form().max_length(MAX_UPLOAD_SIZE))
        .then(upload)
        .map(api::Reply::from)
}

async fn extract_content(mut form_data: FormData) -> ApiResult<(Vec<u8>, MimeType)> {
    while let Some(Ok(part)) = form_data.next().await {
        if part.name() != "content" {
            continue;
        }

        // Ensure file extension matches content type
        let mime_type = MimeType::from_str(part.content_type().unwrap_or("")).map_err(Box::from)?;
        let filename = std::path::Path::new(part.filename().unwrap_or(""));
        let extension = filename.extension().and_then(|ext| ext.to_str()).unwrap_or("");
        if MimeType::from_extension(extension) != Ok(mime_type) {
            return Err(api::Error::ContentTypeMismatch);
        }

        let data = part
            .stream()
            .try_fold(Vec::new(), |mut acc, buf| async move {
                acc.extend_from_slice(buf.chunk());
                Ok(acc)
            })
            .await
            .map_err(|_| api::Error::FailedUpload)?;

        return Ok((data, mime_type));
    }
    Err(api::Error::MissingFormData)
}

#[derive(Serialize)]
struct UploadResponse {
    token: String,
}

async fn upload(auth: AuthResult, form_data: FormData) -> ApiResult<UploadResponse> {
    let client = auth?;
    api::verify_privilege(client.as_ref(), config::privileges().upload_create)?;

    // Set up temp directory if necessary
    filesystem::create_dir(Directory::TemporaryUploads)?;

    let (data, content_type) = extract_content(form_data).await?;
    let upload_token = filesystem::save_uploaded_file(data, content_type)?;

    Ok(UploadResponse { token: upload_token })
}
