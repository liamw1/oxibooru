use crate::api::{self, ApiResult};
use crate::model::enums::{MimeType, PostType};
use image::{DynamicImage, ImageFormat, ImageReader, ImageResult, Limits, Rgb, RgbImage};
use std::io::Cursor;
use std::path::Path;
use video_rs::ffmpeg::format::Pixel;
use video_rs::ffmpeg::media::Type;
use video_rs::Decoder;

pub fn representative_image(file_contents: &[u8], file_path: &Path, content_type: MimeType) -> ApiResult<DynamicImage> {
    match PostType::from(content_type) {
        PostType::Image | PostType::Animation => {
            let image_format = content_type
                .to_image_format()
                .expect("Mime type should be convertable to image format");
            image(file_contents, image_format).map_err(api::Error::from)
        }
        PostType::Video => video_frame(file_path).map_err(api::Error::from),
    }
}

pub fn has_audio(path: &Path) -> Result<bool, video_rs::Error> {
    video_rs::ffmpeg::format::input(path)
        .map(|context| context.streams().best(Type::Audio).is_some())
        .map_err(video_rs::Error::from)
}

/*
    Decodes a raw array of bytes into pixel data.
*/
fn image(bytes: &[u8], format: ImageFormat) -> ImageResult<DynamicImage> {
    let mut reader = ImageReader::new(Cursor::new(bytes));
    reader.set_format(format);
    reader.limits(image_reader_limits());
    reader.decode()
}

/*
    Decodes first frame of video contents
*/
fn video_frame(path: &Path) -> Result<DynamicImage, video_rs::Error> {
    let mut decoder = Decoder::new(path)?;
    let frame = decoder.decode_raw()?;

    let frame_data = frame.data(0);
    let width = frame.width();
    let height = frame.height();
    let stride = frame.stride(0);
    Ok(match frame.format() {
        Pixel::RGB24 => rgb24_frame(frame_data, width, height, stride),
        // There's a looooooot of pixel formats, so I'll just implementment them as they come up
        format => panic!("Video frame format {format:?} is unimplemented!"),
    })
}

fn image_reader_limits() -> Limits {
    const GB: u64 = 1024_u64.pow(3);

    let mut limits = Limits::no_limits();
    limits.max_alloc = Some(4 * GB);
    limits
}

fn rgb24_frame(data: &[u8], width: u32, height: u32, stride: usize) -> DynamicImage {
    let rgb_image = RgbImage::from_fn(width, height, |x, y| {
        let offset = y as usize * stride + x as usize * 3;
        Rgb([data[offset], data[offset + 1], data[offset + 2]])
    });
    DynamicImage::ImageRgb8(rgb_image)
}
