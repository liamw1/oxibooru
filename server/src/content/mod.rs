use crate::api::{ApiError, ApiResult};
use crate::app::AppState;
use crate::config::Config;
use crate::content::cache::CachedProperties;
use crate::content::thumbnail::ThumbnailType;
use crate::filesystem::{self, Directory};
use crate::model::enums::MimeType;
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
    pub fn from_token(config: &Config, token: &str) -> ApiResult<Self> {
        let (_uuid, extension) = token.split_once('.').unwrap_or((token, ""));
        let mime_type = MimeType::from_extension(extension)?;

        let temp_path = config.path(Directory::TemporaryUploads).join(token);
        let data = std::fs::read(&temp_path)?;

        Ok(Self { data, mime_type })
    }

    /// Saves file data to temporary upload directory and returns the name of the file written.
    pub fn save(&self, config: &Config) -> std::io::Result<String> {
        filesystem::save_uploaded_file(config, &self.data, self.mime_type)
    }
}

/// Contains either the name of a file uploaded to the temporary uploads
/// directory, a url pointing to a file on the web, or the contents of the
/// file sent via a multipart request.
///
/// Methods on this object consume it and will save the content to the
/// temporary uploads directory (if not already present) before operating on it.
/// This is because some operations such as video decoding require a path to the
/// content on disk.
pub enum Content {
    DirectUpload(FileContents),
    Token(String),
    Url(Url),
}

impl Content {
    /// Constructs a new [`Content`] from either an in-memory `direct_upload`, a `token` which represents
    /// a file in the temporary uploads directory, or a URL to download the content from.
    ///
    /// If multiple ways of retrieving content are given, the method of retrieving the content will
    /// be the first argument that is not [`None`].
    pub fn new(direct_upload: Option<FileContents>, token: Option<String>, url: Option<Url>) -> Option<Self> {
        match (direct_upload, token, url) {
            (Some(file), _, _) => Some(Self::DirectUpload(file)),
            (None, Some(token), _) => Some(Self::Token(token)),
            (None, None, Some(url)) => Some(Self::Url(url)),
            (None, None, None) => None,
        }
    }

    /// Saves content to temporary uploads directory and returns the name of the file written.
    pub async fn save(self, config: &Config) -> ApiResult<String> {
        match self {
            Self::DirectUpload(file_contents) => file_contents.save(config).map_err(ApiError::from),
            Self::Token(token) => Ok(token),
            Self::Url(url) => download::from_url(config, url).await,
        }
    }

    /// Computes thumbnail for uploaded content.
    pub async fn thumbnail(self, config: &Config, thumbnail_type: ThumbnailType) -> ApiResult<DynamicImage> {
        let token = self.save(config).await?;
        let file_contents = FileContents::from_token(config, &token)?;
        let temp_path = config.path(Directory::TemporaryUploads).join(&token);
        decode::representative_image(config, &file_contents, &temp_path)
            .map(|image| thumbnail::create(config, &image, thumbnail_type))
    }

    /// Computes properties for uploaded content.
    pub async fn compute_properties(self, state: &AppState) -> ApiResult<CachedProperties> {
        let token = self.save(&state.config).await?;
        cache::compute_properties(state, token)
    }

    /// Retrieves content properties from cache or computes them if not present in cache.
    pub async fn get_or_compute_properties(self, state: &AppState) -> ApiResult<CachedProperties> {
        let token = self.save(&state.config).await?;
        cache::get_or_compute_properties(state, token)
    }
}
