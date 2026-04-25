use crate::api::error::{ApiError, ApiResult};
use crate::app::AppState;
use crate::config::Config;
use crate::content::cache::CachedProperties;
use crate::content::thumbnail::ThumbnailType;
use crate::content::upload::UploadToken;
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

/// Contains either the name of a file uploaded to the temporary uploads
/// directory or a url pointing to a file on the web.
///
/// Methods on this object consume it and will save the content to the
/// temporary uploads directory (if not already present) before operating on it.
/// This is because some operations such as video decoding require a path to the
/// content on disk.
pub enum Content {
    Token(UploadToken),
    Url(Url),
}

impl Content {
    /// Constructs a new [`Content`] from either an in-memory `direct_upload`, a `token` which represents
    /// a file in the temporary uploads directory, or a URL to download the content from.
    ///
    /// If multiple ways of retrieving content are given, the method of retrieving the content will
    /// be the first argument that is not [`None`].
    pub fn new(token: Option<UploadToken>, url: Option<Url>) -> Option<Self> {
        match (token, url) {
            (Some(token), _) => Some(Self::Token(token)),
            (None, Some(url)) => Some(Self::Url(url)),
            (None, None) => None,
        }
    }

    /// Saves content to temporary uploads directory and returns the name of the file written.
    pub async fn save(self, config: &Config) -> ApiResult<UploadToken> {
        match self {
            Self::Token(token) => Ok(token),
            Self::Url(url) => download::from_url(config, url).await,
        }
    }

    /// Computes thumbnail for uploaded content.
    pub async fn thumbnail(self, config: &Config, thumbnail_type: ThumbnailType) -> ApiResult<DynamicImage> {
        let token = self.save(config).await?;
        let temp_path = token.path(config);
        tokio::task::block_in_place({
            || {
                decode::representative_image(config, &temp_path, token.mime_type())
                    .map(|image| thumbnail::create(config, &image, thumbnail_type))
            }
        })
    }

    /// Computes properties for uploaded content.
    pub async fn compute_properties(self, state: &AppState) -> ApiResult<CachedProperties> {
        let token = self.save(&state.config).await?;
        tokio::task::block_in_place(|| cache::compute_properties(state, token))
    }

    /// Retrieves content properties from cache or computes them if not present in cache.
    pub async fn get_or_compute_properties(self, state: &AppState) -> ApiResult<CachedProperties> {
        let token = self.save(&state.config).await?;
        tokio::task::block_in_place(|| cache::get_or_compute_properties(state, token))
    }
}

fn map_read_result<T>(result: std::io::Result<T>) -> ApiResult<T> {
    result.map_err(|err| {
        if err.kind() == std::io::ErrorKind::NotFound {
            ApiError::InvalidUploadToken
        } else {
            ApiError::from(err)
        }
    })
}
