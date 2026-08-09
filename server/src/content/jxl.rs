use crate::api::error::{ApiError, ApiResult};
use crate::content;
use image::{DynamicImage, GrayAlphaImage, GrayImage, RgbImage, RgbaImage};
use jxl::api::states::WithImageInfo;
use jxl::api::{
    JxlBitstreamInput, JxlColorType, JxlDataFormat, JxlDecoder, JxlDecoderOptions, JxlOutputBuffer, JxlPixelFormat,
    ProcessingResult,
};
use jxl::error::Error;
use jxl::headers::extra_channels::ExtraChannel;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

/// Decodes the first visible frame of the JPEG XL file at the given `file_path`.
pub fn image(file_path: &Path) -> ApiResult<DynamicImage> {
    let file = content::map_read_result(File::open(file_path))?;
    let mut input = BufReader::new(file);

    let mut decoder = jxl_read_info(&mut input)?;
    let info = decoder.basic_info();

    let (width, height) = info.size;
    let has_alpha = info
        .extra_channels
        .iter()
        .any(|channel| channel.ec_type == ExtraChannel::Alpha);
    let grayscale = decoder.current_pixel_format().color_type.is_grayscale();

    let color_type = match (grayscale, has_alpha) {
        (false, false) => JxlColorType::Rgb,
        (false, true) => JxlColorType::Rgba,
        (true, false) => JxlColorType::Grayscale,
        (true, true) => JxlColorType::GrayscaleAlpha,
    };
    let samples_per_pixel = color_type.samples_per_pixel();
    decoder.set_pixel_format(JxlPixelFormat {
        color_type,
        // u8 output keeps buffer rows free of alignment requirements
        color_data_format: Some(JxlDataFormat::U8 { bit_depth: 8 }),
        // None ignores non-color extra channels (depth, spot colors, ...)
        extra_channel_format: vec![None; info.extra_channels.len()],
    });

    // Advance to the first frame
    let decoder = match decoder.process(&mut input)? {
        ProcessingResult::Complete { result } => result,
        ProcessingResult::NeedsMoreInput { size_hint, .. } => {
            return Err(ApiError::JxlDecoding(Error::OutOfBounds(size_hint)));
        }
    };

    let bytes_per_row = width * samples_per_pixel;
    let mut pixel_data = vec![0; bytes_per_row * height];
    // One buffer for the interleaved color channels; ignored extra channels need none
    let mut buffers = [JxlOutputBuffer::new(&mut pixel_data, height, bytes_per_row)];
    // Decode the frame's pixels; for animations this stops after the first frame
    match decoder.process(&mut input, &mut buffers)? {
        ProcessingResult::Complete { .. } => {}
        ProcessingResult::NeedsMoreInput { size_hint, .. } => {
            return Err(ApiError::JxlDecoding(Error::OutOfBounds(size_hint)));
        }
    }

    let buffer_len = pixel_data.len();
    let (width, height) = (u32::try_from(width).expect(CAST_MESSAGE), u32::try_from(height).expect(CAST_MESSAGE));
    match color_type {
        JxlColorType::Rgb => RgbImage::from_raw(width, height, pixel_data).map(DynamicImage::ImageRgb8),
        JxlColorType::Rgba => RgbaImage::from_raw(width, height, pixel_data).map(DynamicImage::ImageRgba8),
        JxlColorType::Grayscale => GrayImage::from_raw(width, height, pixel_data).map(DynamicImage::ImageLuma8),
        JxlColorType::GrayscaleAlpha => {
            GrayAlphaImage::from_raw(width, height, pixel_data).map(DynamicImage::ImageLumaA8)
        }
        JxlColorType::Bgr | JxlColorType::Bgra => unreachable!("Unrequested JPEG XL color type"),
    }
    .ok_or(ApiError::FrameBufferMismatch(width, height, buffer_len))
}

/// Returns whether the JPEG XL file declares an animation, based on the image header.
pub fn is_animated(path: &Path) -> ApiResult<bool> {
    let file = content::map_read_result(File::open(path))?;
    let mut input = BufReader::new(file);
    jxl_read_info(&mut input).map(|decoder| decoder.basic_info().animation.is_some())
}

const CAST_MESSAGE: &str = "JPEG XL level 10 caps dimensions at 2^30, so u32 cannot overflow";

/// Parses input up to the image header, returning a decoder ready to report
/// image info. Fails if the input ends before the header is complete.
fn jxl_read_info<In: JxlBitstreamInput>(input: &mut In) -> ApiResult<JxlDecoder<WithImageInfo>> {
    let decoder = JxlDecoder::new(JxlDecoderOptions::default());
    match decoder.process(input)? {
        ProcessingResult::Complete { result } => Ok(result),
        // The full file is available, so needing more input means it's truncated
        ProcessingResult::NeedsMoreInput { size_hint, .. } => Err(ApiError::JxlDecoding(Error::OutOfBounds(size_hint))),
    }
}
