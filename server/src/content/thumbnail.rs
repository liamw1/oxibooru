use crate::api::ApiResult;
use crate::content::decode;
use crate::model::enums::MimeType;
use crate::{config, filesystem};
use image::DynamicImage;

pub enum ThumbnailType {
    Post,
    Avatar,
}

pub enum ThumbnailCategory {
    Generated,
    Custom,
}

/// Returns a thumbnail of the given `image`. The size of the thumbnail depends on the `thumbnail_type`
/// and the thumbnail settings in config.toml.
pub fn create(image: &DynamicImage, thumbnail_type: ThumbnailType) -> DynamicImage {
    let thumbnail_width = match thumbnail_type {
        ThumbnailType::Post => config::get().thumbnails.post_width,
        ThumbnailType::Avatar => config::get().thumbnails.avatar_width,
    };
    let thumbnail_height = match thumbnail_type {
        ThumbnailType::Post => config::get().thumbnails.post_height,
        ThumbnailType::Avatar => config::get().thumbnails.avatar_height,
    };
    image.resize_to_fill(thumbnail_width, thumbnail_height, image::imageops::FilterType::Gaussian)
}

/// Returns a thumbnail of the content referred to by `token`. The size of the thumbnail depends on the `thumbnail_type`
/// and the thumbnail settings in config.toml.
pub fn create_from_token(token: &str, thumbnail_type: ThumbnailType) -> ApiResult<DynamicImage> {
    let temp_path = filesystem::temporary_upload_filepath(token);
    let file_contents = std::fs::read(&temp_path)?;

    let (_uuid, extension) = token.split_once('.').unwrap();
    let mime_type = MimeType::from_extension(extension)?;

    decode::representative_image(&file_contents, Some(temp_path), mime_type).map(|image| create(&image, thumbnail_type))
}
