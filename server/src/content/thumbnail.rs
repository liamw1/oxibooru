use crate::config;
use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView};

pub enum ThumbnailType {
    Post,
    Avatar,
}

#[derive(Clone, Copy)]
pub enum ThumbnailCategory {
    Generated,
    Custom,
}

/// Returns a thumbnail of the given `image`. Resizes thumbnail so that it is a small as possible
/// while containing an area with dimensions X_width by X_height, where "X" refers to the thumbnail type.
/// The values of X_width and X_height are read from the config.toml.
pub fn create(image: &DynamicImage, thumbnail_type: ThumbnailType) -> DynamicImage {
    let (config_width, config_height) = match thumbnail_type {
        ThumbnailType::Post => config::get().thumbnails.post_dimensions(),
        ThumbnailType::Avatar => config::get().thumbnails.avatar_dimensions(),
    };

    let (image_width, image_height) = image.dimensions();
    let (thumbnail_width, thumbnail_height) = if image_width > image_height {
        // Thumbnail width is config_height * aspect_ratio
        let width = u64::from(image_width) * u64::from(config_height) / u64::from(image_height);
        let height = u64::from(config_height);
        (width, height)
    } else {
        // Thumbnail height is config_width / aspect_ratio
        let width = u64::from(config_width);
        let height = u64::from(image_height) * u64::from(config_width) / u64::from(image_width);
        (width, height)
    };
    image.resize(thumbnail_width as u32, thumbnail_height as u32, FilterType::Gaussian)
}
