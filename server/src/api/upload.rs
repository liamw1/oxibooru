use crate::api::{ApiResult, AuthResult};
use crate::model::enums::MimeType;
use crate::{api, config, filesystem};
use futures::{StreamExt, TryStreamExt};
use serde::Serialize;
use std::convert::Infallible;
use std::path::Path;
use std::str::FromStr;
use uuid::Uuid;
use warp::multipart::FormData;
use warp::{Buf, Filter, Rejection, Reply};

pub fn routes() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    let upload = warp::post()
        .and(warp::path!("uploads"))
        .and(api::auth())
        .and(warp::filters::multipart::form().max_length(None)) // TODO
        .and_then(upload_endpoint);

    upload
}

#[derive(Serialize)]
struct UploadResponse {
    token: String,
}

async fn upload_endpoint(auth: AuthResult, form: FormData) -> Result<api::Reply, Infallible> {
    Ok(upload(auth, form).await.into())
}

// TODO: Cleanup on failure
async fn upload(auth: AuthResult, form: FormData) -> ApiResult<UploadResponse> {
    let client = auth?;
    api::verify_privilege(client.as_ref(), config::privileges().upload_create)?;

    // Set up temp directory if necessary
    let temp_path = filesystem::temporary_upload_directory();
    if !temp_path.exists() {
        std::fs::create_dir(&temp_path)?;
    }

    // Parse first part and ensure file extension matches content type
    let part = form.into_stream().next().await.ok_or(api::Error::BadMultiPartForm)??;
    let content_type = MimeType::from_str(part.content_type().unwrap_or("")).map_err(Box::from)?;
    let filename = Path::new(part.filename().unwrap_or(""));
    if filename.extension().map(|ext| ext.to_str().unwrap_or("")) != Some(content_type.extension()) {
        return Err(api::Error::ContentTypeMismatch);
    }

    // Give content a temporary handle and write it to disk
    let upload_token = format!("{}.{}", Uuid::new_v4(), content_type.extension());
    let upload_path = filesystem::temporary_upload_filepath(&upload_token);
    let data = part
        .stream()
        .try_fold(Vec::new(), |mut acc, buf| async move {
            acc.extend_from_slice(buf.chunk());
            Ok(acc)
        })
        .await
        .expect("Folding error");
    std::fs::write(&upload_path, &data)?;

    Ok(UploadResponse { token: upload_token })
}
