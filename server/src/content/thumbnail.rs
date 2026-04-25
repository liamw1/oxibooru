use crate::config::Config;
use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView, Pixel, Rgba, RgbaImage};

#[derive(Clone, Copy)]
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
/// while containing an area with dimensions `X_width` by `X_height`, where "X" refers to the thumbnail type.
/// The values of `X_width` and `X_height` are read from the config.toml.
pub fn create(config: &Config, image: &DynamicImage, thumbnail_type: ThumbnailType) -> DynamicImage {
    let (config_width, config_height) = match thumbnail_type {
        ThumbnailType::Post => config.thumbnails.post_dimensions(),
        ThumbnailType::Avatar => config.thumbnails.avatar_dimensions(),
    };

    let (image_width, image_height) = image.dimensions();
    let (thumbnail_width, thumbnail_height) = if image_width > image_height {
        // Thumbnail width is config_height * aspect_ratio
        let width = image_width * config_height / image_height;
        let height = config_height;
        (width, height)
    } else {
        // Thumbnail height is config_width / aspect_ratio
        let width = config_width;
        let height = image_height * config_width / image_width;
        (width, height)
    };
    let resized_image = image.resize(thumbnail_width, thumbnail_height, FilterType::Gaussian);

    // JPEG doesn't support transparency, so composite any transparent pixels
    // onto a solid background to avoid corruption in the final thumbnail.
    if resized_image.color().has_alpha() {
        let mut canvas = RgbaImage::from_pixel(thumbnail_width, thumbnail_height, Rgba([255, 255, 255, 255]));
        for (canvas_pixel, image_pixel) in canvas.pixels_mut().zip(resized_image.to_rgba8().pixels()) {
            canvas_pixel.blend(image_pixel);
        }
        DynamicImage::ImageRgba8(canvas)
    } else {
        resized_image
    }
}
