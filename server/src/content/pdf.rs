use crate::api::error::{ApiError, ApiResult};
use crate::config::Config;
use crate::content;
use hayro::hayro_interpret::InterpreterSettings;
use hayro::hayro_syntax::Pdf;
use hayro::{RenderCache, RenderSettings, render};
use image::{DynamicImage, RgbaImage};
use std::path::Path;
use vello_cpu::color::palette::css::WHITE;

pub fn pdf_preview_image(config: &Config, file_path: &Path) -> ApiResult<DynamicImage> {
    let pdf = {
        let file_contents = content::map_read_result(std::fs::read(file_path))?;
        Pdf::new(file_contents).map_err(crate::model::enums::PdfLoadError)?
    };

    let interpreter_settings = InterpreterSettings { ..Default::default() };

    let page = pdf
        .pages()
        .first()
        .ok_or(ApiError::FromStr("Failed to get page 1 from PDF".into()))?;

    let (dimensions, ratio) = {
        let dimensions = page.render_dimensions();

        let max_size = (f32::from(config.limits.max_pdf_width), f32::from(config.limits.max_pdf_height));

        let ratios = (max_size.0 / dimensions.0, max_size.1 / dimensions.1);

        // find the min ratio to scale down the image while maintaining aspect ratio
        let ratio = f32::min(ratios.0, ratios.1);

        // ensure ratio is at most 1.0. We only want to downscale, not upscale.
        let ratio = f32::min(1.0, ratio);

        ((dimensions.0 * ratio, dimensions.1 * ratio), ratio)
    };

    let dimensions: (u16, u16) = (
        num_traits::cast(dimensions.0).ok_or(ApiError::FromStr("Failed to render PDF".into()))?,
        num_traits::cast(dimensions.1).ok_or(ApiError::FromStr("Failed to render PDF".into()))?,
    );

    let render_settings = RenderSettings {
        x_scale: ratio,
        y_scale: ratio,
        width: Some(dimensions.0),
        height: Some(dimensions.1),
        bg_color: WHITE,
    };
    let cache = RenderCache::new();

    let pixmap = render(page, &cache, &interpreter_settings, &render_settings);

    let png = pixmap.data_as_u8_slice();

    Ok(DynamicImage::ImageRgba8(
        RgbaImage::from_raw(u32::from(pixmap.width()), u32::from(pixmap.height()), png.to_vec())
            .ok_or(ApiError::FromStr("Failed to render PDF".into()))?,
    ))
}
