use image::{DynamicImage, ImageFormat, ImageReader, ImageResult, Limits, Rgb, RgbImage};
use std::io::Cursor;
use std::path::Path;
use video_rs::ffmpeg::format::Pixel;
use video_rs::ffmpeg::media::Type;
use video_rs::Decoder;

/*
    Decodes a raw array of bytes into pixel data.
*/
pub fn image(bytes: &[u8], format: ImageFormat) -> ImageResult<DynamicImage> {
    let mut reader = ImageReader::new(Cursor::new(bytes));
    reader.set_format(format);
    reader.limits(image_reader_limits());
    reader.decode()
}

pub fn video_frame(path: &Path) -> Result<DynamicImage, video_rs::Error> {
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

pub fn has_audio(path: &Path) -> Result<bool, video_rs::Error> {
    video_rs::ffmpeg::format::input(path)
        .map(|context| context.streams().best(Type::Audio).is_some())
        .map_err(video_rs::Error::from)
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
