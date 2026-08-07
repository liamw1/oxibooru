use crate::api::error::{ApiError, ApiResult};
use crate::content::{self, flash};
use crate::model::enums::{MimeType, PostType};
use hayro::hayro_interpret::InterpreterSettings;
use hayro::hayro_interpret::font::{FontData, FontQuery, StandardFont};
use hayro::hayro_interpret::hayro_cmap::CidFamily;
use hayro::hayro_syntax::Pdf;
use hayro::{RenderCache, RenderSettings, render};
use image::{DynamicImage, RgbaImage};
use std::borrow::Cow;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::mpsc::{RecvTimeoutError, SyncSender};
use vello_cpu::color::palette::css::WHITE;

pub fn pdf_representative_image(file_path: &Path) -> ApiResult<DynamicImage> {
    let mut file = Vec::new();
    File::open(file_path)?
        .read_to_end(&mut file)
        .map_err(|_| ApiError::FromStr("Failed to render PDF".into()))?;

    let pdf = Pdf::new(file).map_err(|_| ApiError::FromStr("Failed to render PDF".into()))?;

    let interpreter_settings = InterpreterSettings { ..Default::default() };

    let render_settings = RenderSettings {
        x_scale: 1.0,
        y_scale: 1.0,
        bg_color: WHITE,
        ..Default::default()
    };
    let cache = RenderCache::new();

    let pixmap = render(
        pdf.pages()
            .get(0)
            .ok_or(ApiError::FromStr("Failed to get page 1 from PDF".into()))?,
        &cache,
        &interpreter_settings,
        &render_settings,
    );

    let png = pixmap.data_as_u8_slice();

    Ok(DynamicImage::ImageRgba8(
        RgbaImage::from_raw(pixmap.width() as u32, pixmap.height() as u32, png.to_vec())
            .ok_or(ApiError::FromStr("Failed to render PDF".into()))?,
    ))

    // let mut reader = ImageReader::new(BufReader::new(png.as_slice()));
    // reader.set_format(ImageFormat::Png);
    // reader.limits(Limits::default());
    // reader.decode().map_err(ApiError::from)
}
