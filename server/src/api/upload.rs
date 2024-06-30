use crate::api;
use crate::config;
use crate::model::enums::MimeType;
use crate::model::user::User;
use futures::{StreamExt, TryStreamExt};
use serde::Serialize;
use std::convert::Infallible;
use std::path::PathBuf;
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

async fn upload_endpoint(auth_result: api::AuthenticationResult, form: FormData) -> Result<api::Reply, Infallible> {
    let client = match auth_result {
        Ok(authenticated_user) => authenticated_user,
        Err(err) => {
            let result: Result<(), _> = Err(err);
            return Ok(result.into());
        }
    };

    Ok(upload(form, client.as_ref()).await.into())
}

// TODO: Cleanup on failure
async fn upload(form: FormData, client: Option<&User>) -> Result<UploadResponse, api::Error> {
    api::verify_privilege(api::client_access_level(client), "uploads:create")?;

    // Set up temp directory if necessary
    let data_directory = config::read_required_string("data_dir");
    let temp_path = PathBuf::from(format!("{data_directory}/temporary-uploads"));
    if !temp_path.exists() {
        std::fs::create_dir(&temp_path)?;
    }

    // Parse first part and ensure file extension matches content type
    let part = form.into_stream().next().await.ok_or(api::Error::BadMultiPartForm)??;
    let content_type = MimeType::from_str(part.content_type().unwrap_or(""))?;
    let filename = PathBuf::from(part.filename().unwrap_or(""));
    if filename.extension().map(|ext| ext.to_str().unwrap_or("")) != Some(content_type.extension()) {
        return Err(api::Error::ContentTypeMismatch);
    }

    // Give content a temporary handle and write it to disk
    let upload_token = format!("{}.{}", Uuid::new_v4(), content_type.extension());
    let mut path = temp_path.clone();
    path.push(&upload_token);
    let data = part
        .stream()
        .try_fold(Vec::new(), |mut acc, buf| async move {
            acc.extend_from_slice(buf.chunk());
            Ok(acc)
        })
        .await
        .expect("Folding error");
    std::fs::write(&path, &data)?;

    Ok(UploadResponse { token: upload_token })
}
