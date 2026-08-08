use crate::api::error::{ApiError, ApiResult};
use hayro::hayro_interpret::InterpreterSettings;
use hayro::hayro_syntax::Pdf;
use hayro::{RenderCache, RenderSettings, render};
use image::{DynamicImage, RgbaImage};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use vello_cpu::color::palette::css::WHITE;

pub fn pdf_representative_image(file_path: &Path) -> ApiResult<DynamicImage> {
    let mut file = Vec::new();
    File::open(file_path)?
        .read_to_end(&mut file)
        .map_err(|_| ApiError::FromStr("Failed to render PDF".into()))?;

    let pdf = Pdf::new(file).map_err(|_| ApiError::FromStr("Failed to render PDF".into()))?;

    let interpreter_settings = InterpreterSettings { ..Default::default() };

    let page = pdf
        .pages()
        .get(0)
        .ok_or(ApiError::FromStr("Failed to get page 1 from PDF".into()))?;

    let (dimensions, ratio) = {
        let dimensions = page.render_dimensions();

        let max_size = 1000.0;

        if dimensions.0 <= max_size && dimensions.1 <= max_size {
            (dimensions, 1.0)
        } else {
            let longest_side = dimensions.0.max(dimensions.1);
            let ratio = max_size / longest_side;

            ((dimensions.0 * ratio, dimensions.1 * ratio), ratio)
        }
    };

    let render_settings = RenderSettings {
        x_scale: ratio,
        y_scale: ratio,
        width: Some(dimensions.0 as u16),
        height: Some(dimensions.1 as u16),
        bg_color: WHITE,
        ..Default::default()
    };
    let cache = RenderCache::new();

    let pixmap = render(page, &cache, &interpreter_settings, &render_settings);

    let png = pixmap.data_as_u8_slice();

    Ok(DynamicImage::ImageRgba8(
        RgbaImage::from_raw(pixmap.width() as u32, pixmap.height() as u32, png.to_vec())
            .ok_or(ApiError::FromStr("Failed to render PDF".into()))?,
    ))
}
