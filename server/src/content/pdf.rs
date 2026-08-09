use crate::api::error::{ApiError, ApiResult};
use crate::config::Config;
use crate::content;
use hayro::hayro_interpret::InterpreterSettings;
use hayro::hayro_syntax::Pdf;
use hayro::vello_cpu::color::palette::css::WHITE;
use hayro::{RenderCache, RenderSettings, render};
use image::error::LimitErrorKind;
use image::{DynamicImage, RgbaImage};
use std::path::Path;

pub fn pdf_preview_image(config: &Config, file_path: &Path) -> ApiResult<DynamicImage> {
    let pdf = {
        let file_contents = content::map_read_result(std::fs::read(file_path))?;
        Pdf::new(file_contents).map_err(crate::model::enums::PdfLoadError)?
    };

    let page = pdf.pages().first().ok_or(ApiError::EmptyPdf)?;

    let (page_width, page_height, ratio) = {
        let dimensions = page.render_dimensions();

        let max_size = (f32::from(config.limits.max_pdf_width), f32::from(config.limits.max_pdf_height));

        let ratios = (max_size.0 / dimensions.0, max_size.1 / dimensions.1);

        // find the min ratio to scale down the image while maintaining aspect ratio
        let ratio = f32::min(ratios.0, ratios.1);

        // ensure ratio is at most 1.0. We only want to downscale, not upscale.
        let ratio = f32::min(1.0, ratio);

        (dimensions.0 * ratio, dimensions.1 * ratio, ratio)
    };

    let page_width = num_traits::cast(page_width).ok_or(LimitErrorKind::DimensionError)?;
    let page_height = num_traits::cast(page_height).ok_or(LimitErrorKind::DimensionError)?;

    let render_settings = RenderSettings {
        x_scale: ratio,
        y_scale: ratio,
        width: Some(page_width),
        height: Some(page_height),
        bg_color: WHITE,
    };

    let pixmap = render(page, &RenderCache::new(), &InterpreterSettings::default(), &render_settings);
    let width = pixmap.width();
    let height = pixmap.height();

    // There should be some way to do this without reallocating,
    // Vec<PreMulRgba8> is the same bitwise as Vec<u8> (except for num elements)
    let rgba_vec = pixmap
        .take()
        .into_iter()
        .map(|c| c.to_u8_array())
        .flatten()
        .collect::<Vec<u8>>();

    let size = rgba_vec.len();

    Ok(DynamicImage::ImageRgba8(
        RgbaImage::from_raw(u32::from(width), u32::from(height), rgba_vec).ok_or(ApiError::FrameBufferMismatch(
            width as u32,
            height as u32,
            size as usize,
        ))?,
    ))
}
