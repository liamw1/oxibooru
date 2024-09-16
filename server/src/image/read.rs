use crate::model::enums::MimeType;
use image::{DynamicImage, ImageFormat, ImageReader, ImageResult, Limits};
use std::io::Cursor;

pub fn decode_image(bytes: &[u8], mime_type: MimeType) -> ImageResult<DynamicImage> {
    let mut reader = ImageReader::new(Cursor::new(bytes));
    reader.set_format(image_reader_format(mime_type));
    reader.limits(image_reader_limits());
    reader.decode()
}

fn image_reader_format(mime_type: MimeType) -> ImageFormat {
    match mime_type {
        MimeType::Bmp => ImageFormat::Bmp,
        MimeType::Gif => ImageFormat::Gif,
        MimeType::Jpeg => ImageFormat::Jpeg,
        MimeType::Png => ImageFormat::Png,
        MimeType::Webp => ImageFormat::WebP,
        MimeType::Mov | MimeType::Mp4 | MimeType::Webm => panic!("Not an image format"),
    }
}

fn image_reader_limits() -> Limits {
    const GB: u64 = 1024 * 1024 * 1024;

    let mut limits = Limits::no_limits();
    limits.max_alloc = Some(4 * GB);
    limits
}
