use crate::api::ApiResult;
use crate::content::decode;
use crate::model::enums::{MimeType, PostType};
use crate::{config, filesystem};
use image::DynamicImage;

pub fn create(image: &DynamicImage) -> DynamicImage {
    image.resize_to_fill(
        config::get().thumbnails.post_width,
        config::get().thumbnails.post_height,
        image::imageops::FilterType::Gaussian,
    )
}

pub fn create_from_token(token: &str) -> ApiResult<DynamicImage> {
    let temp_path = filesystem::temporary_upload_filepath(token);
    let file_contents = std::fs::read(&temp_path)?;

    let (_uuid, extension) = token.split_once('.').unwrap();
    let mime_type = MimeType::from_extension(extension)?;

    let image = match PostType::from(mime_type) {
        PostType::Image | PostType::Animation => {
            let image_format = mime_type
                .to_image_format()
                .expect("Mime type should be convertable to image format");
            decode::image(&file_contents, image_format)?
        }
        PostType::Video => decode::video_frame(&temp_path)?,
    };

    Ok(create(&image))
}
