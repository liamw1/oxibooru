use crate::api::ApiResult;
use crate::content::{FileContents, flash};
use crate::model::enums::PostType;
use crate::{api, config};
use image::{DynamicImage, ImageFormat, ImageReader, ImageResult, Limits, Rgb, RgbImage};
use std::fs::File;
use std::io::{BufReader, Cursor};
use std::path::Path;
use swf::Tag;
use tracing::error;
use video_rs::Decoder;
use video_rs::ffmpeg::format::Pixel;
use video_rs::ffmpeg::media::Type;

/// Returns a representative image for the given content.
/// For images, this is simply the decoded image.
/// For videos, it is the first frame of the video.
/// For Flash media, it is the largest image that can be decoded from the Flash tags.
pub fn representative_image(file_contents: &FileContents, file_path: &Path) -> ApiResult<DynamicImage> {
    match PostType::from(file_contents.mime_type) {
        PostType::Image | PostType::Animation => {
            let image_format = file_contents
                .mime_type
                .to_image_format()
                .expect("Mime type should be convertable to image format");
            image(&file_contents.data, image_format).map_err(api::Error::from)
        }
        PostType::Video => video_frame(file_path)
            .map_err(api::Error::from)
            .and_then(|frame| frame.ok_or(api::Error::EmptyVideo)),
        PostType::Flash => flash_image(file_path).and_then(|frame| frame.ok_or(api::Error::EmptySwf)),
    }
}

/// Returns if the video at `path` has an audio channel.
pub fn video_has_audio(path: &Path) -> Result<bool, video_rs::Error> {
    video_rs::ffmpeg::format::input(path)
        .map(|context| context.streams().best(Type::Audio).is_some())
        .map_err(video_rs::Error::from)
}

/// Returns if the swf at `path` has audio.
pub fn swf_has_audio(path: &Path) -> ApiResult<bool> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let swf_buf = swf::decompress_swf(reader)?;
    let swf = swf::parse_swf(&swf_buf)?;

    Ok(swf.tags.iter().any(|tag| {
        matches!(
            tag,
            Tag::DefineButtonSound(_)
                | Tag::DefineSound(_)
                | Tag::SoundStreamBlock(_)
                | Tag::SoundStreamHead(_)
                | Tag::SoundStreamHead2(_)
                | Tag::StartSound(_)
                | Tag::StartSound2 { .. }
        )
    }))
}

/// Decodes a raw array of bytes into pixel data.
pub fn image(bytes: &[u8], format: ImageFormat) -> ImageResult<DynamicImage> {
    let mut reader = ImageReader::new(Cursor::new(bytes));
    reader.set_format(format);
    reader.limits(image_reader_limits());
    reader.decode()
}

/// Decodes first frame of video contents.
fn video_frame(path: &Path) -> Result<Option<DynamicImage>, video_rs::Error> {
    let mut decoder = Decoder::new(path)?;
    let frame = decoder.decode_raw()?;

    if frame.planes() == 0 {
        return Ok(None);
    }

    let frame_data = frame.data(0);
    let width = frame.width();
    let height = frame.height();
    let stride = frame.stride(0);
    Ok(Some(match frame.format() {
        Pixel::RGB24 => rgb24_frame(frame_data, width, height, stride),
        // There's a looooooot of pixel formats, so I'll just implementment them as they come up
        format => panic!("Video frame format {format:?} is unimplemented!"),
    }))
}

/// Search swf tags for the largest decodable image.
fn flash_image(path: &Path) -> ApiResult<Option<DynamicImage>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let swf_buf = swf::decompress_swf(reader)?;
    let swf = swf::parse_swf(&swf_buf)?;

    let encoding_table = swf
        .tags
        .iter()
        .find_map(|tag| {
            if let Tag::JpegTables(table) = tag {
                Some(table)
            } else {
                None
            }
        })
        .copied();
    let mut images: Vec<_> = swf
        .tags
        .iter()
        .filter_map(|tag| match tag {
            Tag::DefineBits { id: _, jpeg_data } => {
                let jpeg_data = flash::glue_tables_to_jpeg(jpeg_data, encoding_table);
                Some(image::load_from_memory_with_format(&jpeg_data, ImageFormat::Jpeg).map_err(flash::Error::from))
            }
            Tag::DefineBitsLossless(bits) => flash::decode_define_bits_lossless(bits).transpose(),
            Tag::DefineBitsJpeg2 { id: _, jpeg_data } => Some(flash::decode_define_bits_jpeg(jpeg_data, None)),
            Tag::DefineBitsJpeg3(bits) => Some(flash::decode_define_bits_jpeg(bits.data, Some(bits.alpha_data))),
            _ => None,
        })
        .filter_map(|image_result| match image_result {
            Ok(image) => Some(image),
            Err(err) => {
                error!("Failure to decode flash image for reason: {err}");
                None
            }
        })
        .collect();

    // Some Flash files only have video frames, which are hard to decode.
    // So, we feed to ffmpeg and see if it can decode a representaive frame.
    if let Ok(Some(frame)) = video_frame(path) {
        images.push(frame);
    }

    // Sort images in order of decreasing effective width after cropping for thumbnails
    images.sort_by_key(|image| {
        let (thumbnail_width, thumbnail_height) = config::get().thumbnails.post_dimensions();

        // Condition is equivalent to image_aspect_ratio > config_thumbnail_aspect_ratio
        let effective_width = match image.width() * thumbnail_height > thumbnail_width * image.height() {
            true => image.height() * thumbnail_width / thumbnail_height,
            false => image.width(),
        };
        u32::MAX - effective_width
    });
    Ok(images.into_iter().next())
}

/// Defines upper limit on decoded image size to prevent crippling the server.
fn image_reader_limits() -> Limits {
    const GB: u64 = 1024_u64.pow(3);

    let mut limits = Limits::no_limits();
    limits.max_alloc = Some(4 * GB);
    limits
}

/// Converts decoded video data into a [`DynamicImage`].
fn rgb24_frame(data: &[u8], width: u32, height: u32, stride: usize) -> DynamicImage {
    let rgb_image = RgbImage::from_fn(width, height, |x, y| {
        let offset = y as usize * stride + x as usize * 3;
        Rgb([data[offset], data[offset + 1], data[offset + 2]])
    });
    DynamicImage::ImageRgb8(rgb_image)
}
