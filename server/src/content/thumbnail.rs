use crate::config::Config;
use image::{DynamicImage, GenericImageView, RgbImage};

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
pub fn create(config: &Config, image: DynamicImage, thumbnail_type: ThumbnailType) -> DynamicImage {
    // JPEG doesn't support transparency, so composite any transparent pixels
    // onto a solid background to avoid corruption in the final thumbnail.
    let composited_image = if image.color().has_alpha() {
        let rgba = image.into_rgba8();
        let mut canvas = RgbImage::new(rgba.width(), rgba.height());
        for (source, destination) in rgba.pixels().zip(canvas.pixels_mut()) {
            // Blend RGBA pixels onto white background
            let [r, g, b, a] = source.0;
            let a = u32::from(a);
            let div255_round = |x: u32| ((x + 127) / 255) as u8;
            destination.0 = [
                div255_round(u32::from(r) * a + 255 * (255 - a)),
                div255_round(u32::from(g) * a + 255 * (255 - a)),
                div255_round(u32::from(b) * a + 255 * (255 - a)),
            ];
        }
        DynamicImage::ImageRgb8(canvas)
    } else {
        image
    };

    let (config_width, config_height) = match thumbnail_type {
        ThumbnailType::Post => config.thumbnails.post_dimensions(),
        ThumbnailType::Avatar => config.thumbnails.avatar_dimensions(),
    };

    // Only the smaller dimension needs to be constrained,
    // DynamicImage::resize() will handle the other dimension based on the aspect ratio.
    // Note that thumbnail_width and thumbnail_height are maximum constraining values for resize(),
    // not the true size of the resulting image.
    let (image_width, image_height) = composited_image.dimensions();
    let (thumbnail_width, thumbnail_height) =
        if u64::from(image_width) * u64::from(config_height) > u64::from(image_height) * u64::from(config_width) {
            (u32::MAX, config_height)
        } else {
            (config_width, u32::MAX)
        };
    composited_image.thumbnail(thumbnail_width, thumbnail_height)
}
