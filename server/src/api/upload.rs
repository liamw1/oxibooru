use crate::api::{ApiResult, AuthResult};
use crate::model::enums::MimeType;
use crate::{api, config, filesystem};
use futures::{StreamExt, TryStreamExt};
use serde::Serialize;
use std::convert::Infallible;
use std::path::Path;
use std::str::FromStr;
use warp::multipart::FormData;
use warp::{Buf, Filter, Rejection, Reply};

pub fn routes() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    warp::post()
        .and(warp::path!("uploads"))
        .and(api::auth())
        .and(warp::filters::multipart::form().max_length(MAX_UPLOAD_SIZE))
        .and_then(upload_endpoint)
}

const MAX_UPLOAD_SIZE: u64 = 4 * 1024 * 1024 * 1024;

#[derive(Serialize)]
struct UploadResponse {
    token: String,
}

async fn upload_endpoint(auth: AuthResult, form: FormData) -> Result<api::Reply, Infallible> {
    Ok(upload(auth, form).await.into())
}

async fn upload(auth: AuthResult, form: FormData) -> ApiResult<UploadResponse> {
    let client = auth?;
    api::verify_privilege(client.as_ref(), config::privileges().upload_create)?;

    // Set up temp directory if necessary
    filesystem::create_dir(filesystem::temporary_upload_directory())?;

    // Parse first part and ensure file extension matches content type
    let part = form.into_stream().next().await.ok_or(api::Error::BadMultiPartForm)??;
    let content_type = MimeType::from_str(part.content_type().unwrap_or("")).map_err(Box::from)?;
    let filename = Path::new(part.filename().unwrap_or(""));
    if MimeType::from_extension(filename.extension().and_then(|ext| ext.to_str()).unwrap_or("")) != Ok(content_type) {
        return Err(api::Error::ContentTypeMismatch);
    }

    // Give content a temporary handle and write it to disk
    let data = part
        .stream()
        .try_fold(Vec::new(), |mut acc, buf| async move {
            acc.extend_from_slice(buf.chunk());
            Ok(acc)
        })
        .await
        .map_err(|_| api::Error::FailedUpload)?;
    let upload_token = filesystem::upload(&data, content_type)?;

    Ok(UploadResponse { token: upload_token })
}
