use crate::api::ApiResult;
use crate::content::cache::CachedProperties;
use crate::content::thumbnail::ThumbnailType;
use crate::model::enums::MimeType;
use crate::{api, filesystem};
use axum::RequestExt;
use axum::extract::{FromRequest, Json, Multipart};
use axum::http::StatusCode;
use axum::http::header::CONTENT_TYPE;
use axum::response::{IntoResponse, Response};
use image::DynamicImage;
use url::Url;

pub mod cache;
pub mod decode;
pub mod download;
mod flash;
pub mod hash;
pub mod signature;
pub mod thumbnail;
pub mod upload;

/// Stores file contents and mime type of an uploaded file.
pub struct FileContents {
    pub data: Vec<u8>,
    pub mime_type: MimeType,
}

impl FileContents {
    /// Constructs an instance from a temporary upload.
    pub fn from_token(token: &str) -> ApiResult<Self> {
        let (_uuid, extension) = token.split_once('.').unwrap();
        let mime_type = MimeType::from_extension(extension)?;

        let temp_path = filesystem::temporary_upload_filepath(token);
        let data = std::fs::read(&temp_path)?;

        Ok(Self { data, mime_type })
    }

    /// Saves file data to temporary upload directory.
    pub fn save(&self) -> std::io::Result<String> {
        filesystem::save_uploaded_file(&self.data, self.mime_type)
    }
}

/// Contains either the name of a file uploaded to the temporary uploads
/// directory, a url pointing to a file on the web, or the contents of the
/// file sent via a multipart request.
pub enum Content {
    DirectUpload(FileContents),
    Token(String),
    Url(Url),
}

impl Content {
    pub fn new(direct_upload: Option<FileContents>, token: Option<String>, url: Option<Url>) -> Option<Self> {
        match (direct_upload, token, url) {
            (Some(file), _, _) => Some(Self::DirectUpload(file)),
            (None, Some(token), _) => Some(Self::Token(token)),
            (None, None, Some(url)) => Some(Self::Url(url)),
            (None, None, None) => None,
        }
    }

    pub async fn save(self) -> ApiResult<String> {
        match self {
            Self::DirectUpload(file_contents) => file_contents.save().map_err(api::Error::from),
            Self::Token(token) => Ok(token),
            Self::Url(url) => download::from_url(url).await,
        }
    }

    pub async fn thumbnail(self, thumbnail_type: ThumbnailType) -> ApiResult<DynamicImage> {
        let token = self.save().await?;
        let file_contents = FileContents::from_token(&token)?;
        let file_path = filesystem::temporary_upload_filepath(&token);
        decode::representative_image(&file_contents, &file_path).map(|image| thumbnail::create(&image, thumbnail_type))
    }

    pub async fn compute_properties(self) -> ApiResult<CachedProperties> {
        let token = self.save().await?;
        cache::compute_properties(token)
    }

    pub async fn get_or_compute_properties(self) -> ApiResult<CachedProperties> {
        let token = self.save().await?;
        cache::get_or_compute_properties(token)
    }
}

pub enum JsonOrMultipart<T> {
    Json(T),
    Multipart(Multipart),
}

impl<S, T> FromRequest<S> for JsonOrMultipart<T>
where
    S: Send + Sync,
    Json<T>: FromRequest<()>,
    T: 'static,
{
    type Rejection = Response;

    async fn from_request(req: axum::extract::Request, _state: &S) -> Result<Self, Self::Rejection> {
        let content_type_header = req.headers().get(CONTENT_TYPE);
        let content_type = content_type_header.and_then(|value| value.to_str().ok());

        if let Some(content_type) = content_type {
            if content_type.starts_with("application/json") {
                let Json(payload) = req.extract().await.map_err(IntoResponse::into_response)?;
                return Ok(Self::Json(payload));
            }
            if content_type.starts_with("multipart/form-data") {
                let payload = req.extract().await.map_err(IntoResponse::into_response)?;
                return Ok(Self::Multipart(payload));
            }
        }
        Err(StatusCode::UNSUPPORTED_MEDIA_TYPE.into_response())
    }
}
