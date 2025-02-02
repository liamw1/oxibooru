use crate::config;
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
