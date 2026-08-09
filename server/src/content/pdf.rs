use crate::api::error::{ApiError, ApiResult};
use crate::config::Config;
use crate::content;
use hayro::hayro_interpret::InterpreterSettings;
use hayro::hayro_syntax::Pdf;
use hayro::hayro_syntax::page::Page;
use hayro::vello_cpu::color::palette::css::WHITE;
use hayro::vello_cpu::color::{AlphaColor, Srgb};
use hayro::{RenderCache, RenderSettings, render};
use image::error::LimitErrorKind;
use image::{DynamicImage, RgbaImage};
use std::path::Path;

struct PdfRenderDimensions {
    pub width: f32,
    pub height: f32,
    pub ratio: f32,
}

impl PdfRenderDimensions {
    // Calculates the dimensions to render a PDF page at, given the config limits and the page's original dimensions.
    fn from_page(config: &Config, page: &Page<'_>) -> Self {
        let (width, height) = page.render_dimensions();

        // find the min ratio to scale down the image while maintaining aspect ratio
        let ratio = {
            let max_width = f32::from(config.limits.max_pdf_width);
            let max_height = f32::from(config.limits.max_pdf_height);

            let width_ratio = max_width / width;
            let height_ratio = max_height / height;

            f32::min(width_ratio, height_ratio)
        };

        // ensure ratio is at most 1.0. We only want to downscale, not upscale.
        let ratio = f32::min(1.0, ratio);

        Self {
            width: width * ratio,
            height: height * ratio,
            ratio: ratio,
        }
    }

    // Returns the render settings for rendering a PDF page at the dimensions specified by this struct.
    fn render_settings(&self, background_color: AlphaColor<Srgb>) -> ApiResult<RenderSettings> {
        Ok(RenderSettings {
            x_scale: self.ratio,
            y_scale: self.ratio,
            width: Some(num_traits::cast(self.width).ok_or(LimitErrorKind::DimensionError)?),
            height: Some(num_traits::cast(self.height).ok_or(LimitErrorKind::DimensionError)?),
            bg_color: background_color,
        })
    }
}

pub fn pdf_preview_image(config: &Config, file_path: &Path) -> ApiResult<DynamicImage> {
    let pdf = {
        let file_contents = content::map_read_result(std::fs::read(file_path))?;
        Pdf::new(file_contents).map_err(crate::model::enums::PdfLoadError)?
    };

    // The preview image is just gonna show the first page
    let page = pdf.pages().first().ok_or(ApiError::EmptyPdf)?;

    let dimensions = PdfRenderDimensions::from_page(config, page);

    let pixmap =
        render(page, &RenderCache::new(), &InterpreterSettings::default(), &dimensions.render_settings(WHITE)?);

    let width = pixmap.width();
    let height = pixmap.height();

    // There should be some way to do this without reallocating,
    // Vec<PreMulRgba8> is the same bitwise as Vec<u8> (except for num elements)
    let rgba_vec = pixmap
        .take()
        .into_iter()
        .flat_map(hayro::vello_cpu::color::PremulRgba8::to_u8_array)
        .collect::<Vec<u8>>();

    let size = rgba_vec.len();

    Ok(DynamicImage::ImageRgba8(
        RgbaImage::from_raw(u32::from(width), u32::from(height), rgba_vec).ok_or(ApiError::FrameBufferMismatch(
            u32::from(width),
            u32::from(height),
            size,
        ))?,
    ))
}
