use crate::api::error::{ApiError, ApiResult};
use crate::config::Config;
use crate::content;
use hayro::hayro_interpret::InterpreterSettings;
use hayro::hayro_syntax::Pdf;
use hayro::hayro_syntax::page::Page;
use hayro::vello_cpu::color::palette::css::WHITE;
use hayro::vello_cpu::color::{AlphaColor, PremulRgba8, Srgb};
use hayro::{RenderCache, RenderSettings};
use image::error::LimitErrorKind;
use image::{DynamicImage, RgbaImage};
use std::path::Path;

pub fn pdf_preview_image(config: &Config, file_path: &Path) -> ApiResult<DynamicImage> {
    let pdf = {
        let file_contents = content::map_read_result(std::fs::read(file_path))?;
        Pdf::new(file_contents)
    }?;
    let page = pdf.pages().first().ok_or(ApiError::EmptyPdf)?; // The preview image will be the first page

    let dimensions = PdfRenderDimensions::from_page(config, page)?;
    let pixmap =
        hayro::render(page, &RenderCache::new(), &InterpreterSettings::default(), &dimensions.render_settings(WHITE));

    let width = u32::from(pixmap.width());
    let height = u32::from(pixmap.height());

    // NOTE: Using the premultipied RGBA8 pixels here for the image buffer is only correct because the
    //       the background color is opaque. If this changes, `take_unpremultiplied` should be used instead.
    let rgba_buffer = into_bytes(pixmap.take());
    let buffer_size = rgba_buffer.len();
    RgbaImage::from_raw(width, height, rgba_buffer)
        .ok_or(ApiError::FrameBufferMismatch(width, height, buffer_size))
        .map(DynamicImage::ImageRgba8)
}

struct PdfRenderDimensions {
    pub width: u16,  // Width (in pixels) of the rendered PDF page.
    pub height: u16, // Height (in pixels) of the rendered PDF page.
    pub scale: f32,  // Scale applied to the original PDF page dimensions to fit within width/height.
}

impl PdfRenderDimensions {
    /// Calculates the dimensions to render a PDF page at, given the config limits and the page's original dimensions.
    fn from_page(config: &Config, page: &Page<'_>) -> ApiResult<Self> {
        let (width, height) = page.render_dimensions();

        // Find the min ratio to scale down the image while maintaining aspect ratio
        let scale = {
            let max_width = f32::from(config.limits.max_pdf_width);
            let max_height = f32::from(config.limits.max_pdf_height);
            f32::min(max_width / width, max_height / height)
        };
        let scale = f32::min(1.0, scale); // Ensure scale is at most 1.0. We only want to downscale, not upscale.

        let width = num_traits::cast(width * scale).ok_or(LimitErrorKind::DimensionError)?;
        let height = num_traits::cast(height * scale).ok_or(LimitErrorKind::DimensionError)?;
        Ok(Self { width, height, scale })
    }

    /// Returns the render settings for rendering a PDF page at the dimensions specified by this struct.
    fn render_settings(&self, background_color: AlphaColor<Srgb>) -> RenderSettings {
        RenderSettings {
            x_scale: self.scale,
            y_scale: self.scale,
            width: Some(self.width),
            height: Some(self.height),
            bg_color: background_color,
        }
    }
}

/// Converts a `Vec<PremulRgba8>` to `Vec<u8>` without copying.
fn into_bytes(rgba_buffer: Vec<PremulRgba8>) -> Vec<u8> {
    const _: () = assert!(size_of::<PremulRgba8>() == 4 * size_of::<u8>(), "PremulRgba8 must be 4 bytes");
    const _: () = assert!(align_of::<PremulRgba8>() == align_of::<u8>(), "PremulRgba8 must have same alignment as u8");
    const _: () = assert!(!std::mem::needs_drop::<PremulRgba8>(), "PremulRgba8 must be trivially destructible");

    let (ptr, len, capacity) = rgba_buffer.into_raw_parts();
    // SAFETY: PremulRgba8 is repr(C), size 4, align 1, and has no padding,
    //         so the transmuted buffer is safe to read and deallocate.
    unsafe { Vec::from_raw_parts(ptr.cast::<u8>(), 4 * len, 4 * capacity) }
}
