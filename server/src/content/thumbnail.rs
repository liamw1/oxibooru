use crate::api::ApiResult;
use crate::content::decode;
use crate::model::enums::MimeType;
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

    decode::representative_image(&file_contents, &temp_path, mime_type).map(|image| create(&image))
}
