use crate::api::ApiResult;
use crate::content::flash;
use crate::model::enums::{MimeType, PostType};
use crate::{api, config, filesystem};
use image::{DynamicImage, ImageFormat, ImageReader, ImageResult, Limits, Rgb, RgbImage};
use std::fs::File;
use std::io::{BufReader, Cursor};
use std::path::{Path, PathBuf};
use swf::Tag;
use video_rs::ffmpeg::format::Pixel;
use video_rs::ffmpeg::media::Type;
use video_rs::Decoder;

/// Returns a representative image for the given content.
/// For images, this is simply the decoded image.
/// For videos, it is the first frame of the video.
pub fn representative_image(
    file_contents: &[u8],
    file_path: Option<PathBuf>,
    content_type: MimeType,
) -> ApiResult<DynamicImage> {
    let path = match file_path {
        Some(path) => path,
        None => {
            let token = filesystem::save_uploaded_file(file_contents, content_type)?;
            filesystem::temporary_upload_filepath(&token)
        }
    };

    match PostType::from(content_type) {
        PostType::Image | PostType::Animation => {
            let image_format = content_type
                .to_image_format()
                .expect("Mime type should be convertable to image format");
            image(file_contents, image_format).map_err(api::Error::from)
        }
        PostType::Video => video_frame(&path).map_err(api::Error::from),
        PostType::Flash => swf_image(&path),
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
    let file = File::open(path).unwrap();
    let reader = BufReader::new(file);
    let swf_buf = swf::decompress_swf(reader).unwrap();
    let swf = swf::parse_swf(&swf_buf).expect("Failed to parse SWF");

    Ok(swf
        .tags
        .iter()
        .position(|tag| match tag {
            Tag::DefineButtonSound(_)
            | Tag::DefineSound(_)
            | Tag::SoundStreamBlock(_)
            | Tag::SoundStreamHead(_)
            | Tag::SoundStreamHead2(_)
            | Tag::StartSound(_)
            | Tag::StartSound2 { .. } => true,
            _ => false,
        })
        .is_some())
}

/// Decodes a raw array of bytes into pixel data.
pub fn image(bytes: &[u8], format: ImageFormat) -> ImageResult<DynamicImage> {
    let mut reader = ImageReader::new(Cursor::new(bytes));
    reader.set_format(format);
    reader.limits(image_reader_limits());
    reader.decode()
}

/// Decodes first frame of video contents.
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

fn swf_image(path: &Path) -> ApiResult<DynamicImage> {
    let file = File::open(path).unwrap();
    let reader = BufReader::new(file);
    let swf_buf = swf::decompress_swf(reader).unwrap();
    let swf = swf::parse_swf(&swf_buf).expect("Failed to parse SWF");

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
                eprintln!("Failure to decode flash image for reason: {err}");
                None
            }
        })
        .collect();

    // Sort images in order of decreasing effective width after cropping for thumbnails
    images.sort_by_key(|image| {
        let thumbnail_aspect_ratio =
            config::get().thumbnails.post_width as f32 / config::get().thumbnails.post_height as f32;
        let image_apsect_ratio = image.width() as f32 / image.height() as f32;

        let effective_width = match image_apsect_ratio > thumbnail_aspect_ratio {
            true => (image.height() as f32 * thumbnail_aspect_ratio) as u32,
            false => image.width(),
        };
        u32::MAX - effective_width
    });
    Ok(images.into_iter().next().unwrap())
}
