use image::{DynamicImage, ImageFormat, ImageReader, ImageResult, Limits};
use std::io::Cursor;

/*
    Decodes a raw array of bytes into pixel data.
*/
pub fn image(bytes: &[u8], format: ImageFormat) -> ImageResult<DynamicImage> {
    let mut reader = ImageReader::new(Cursor::new(bytes));
    reader.set_format(format);
    reader.limits(image_reader_limits());
    reader.decode()
}

fn image_reader_limits() -> Limits {
    const GB: u64 = 1024 * 1024 * 1024;

    let mut limits = Limits::no_limits();
    limits.max_alloc = Some(4 * GB);
    limits
}
