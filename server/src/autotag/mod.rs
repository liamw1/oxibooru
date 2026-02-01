use crate::api::error::{ApiError, ApiResult, OptionalFeature};
use crate::config::{AutoTagConfig, Config};
use crate::filesystem::Directory;
use crate::string::SmallString;
use image::imageops::FilterType;
use image::{DynamicImage, GenericImage, GenericImageView, Pixel, Rgb, RgbImage, Rgba, RgbaImage};
use ort::execution_providers::{CPUExecutionProvider, ExecutionProvider, WebGPUExecutionProvider};
use ort::inputs;
use ort::session::Session;
use ort::value::{Outlet, Tensor};
use std::path::Path;
use strum::FromRepr;
use tokio::sync::Mutex;
use tracing::{error, warn};

pub struct AutoTagSession {
    session: Mutex<Session>,
    labels: Vec<Label>,
    input_name: String,
    target_size: u32,
}

impl AutoTagSession {
    pub fn new(config: &Config) -> ApiResult<Option<Self>> {
        const DEFAULT_TARGET_SIZE: u32 = 448;

        config
            .auto_tag
            .as_ref()
            .map(|auto_tag_config| {
                let labels_path = config.path(Directory::Models).join(&auto_tag_config.labels);
                let labels = load_labels(&labels_path)?;
                let session = create_session(config)?;

                let input_name = session.inputs().first().map_or("", Outlet::name).to_owned();
                let target_size = session
                    .inputs()
                    .first()
                    .and_then(|input| input.dtype().tensor_shape())
                    .and_then(|shape| shape.get(1).copied())
                    .and_then(|size| u32::try_from(size).ok())
                    .unwrap_or(DEFAULT_TARGET_SIZE);

                Ok(Self {
                    session: Mutex::new(session),
                    labels,
                    input_name,
                    target_size,
                })
            })
            .transpose()
    }

    /// Preprocess image for auto-tagger
    pub fn compute_image_tensor_data(&self, image: &DynamicImage) -> ApiResult<Vec<f32>> {
        const WHITE_RGB: Rgb<u8> = Rgb([255, 255, 255]);
        const WHITE_RGBA: Rgba<u8> = Rgba([255, 255, 255, 255]);

        let _timer = crate::time::Timer::new("image_tensor");

        let (image_width, image_height) = image.dimensions();

        // Blend translucent pixels against white canvas
        let mut canvas = RgbaImage::from_pixel(image_width, image_height, WHITE_RGBA);
        for (canvas_pixel, image_pixel) in canvas.pixels_mut().zip(image.to_rgba8().pixels()) {
            canvas_pixel.blend(image_pixel);
        }
        let alpha_composite = DynamicImage::ImageRgba8(canvas);

        // Pad to square
        let max_dim = std::cmp::max(image_width, image_height);
        let pad_left = (max_dim - image_width) / 2;
        let pad_top = (max_dim - image_height) / 2;
        let mut padded_image = RgbImage::from_pixel(max_dim, max_dim, WHITE_RGB);
        padded_image.copy_from(&alpha_composite.to_rgb8(), pad_left, pad_top)?;
        let padded_image = DynamicImage::ImageRgb8(padded_image);

        // Resize to target size
        let resized_image = padded_image.resize(self.target_size, self.target_size, FilterType::CatmullRom);

        // Convert to BGR order, NHWC format
        Ok(resized_image
            .to_rgb8()
            .pixels()
            .flat_map(|&Rgb([r, g, b])| [f32::from(b), f32::from(g), f32::from(r)])
            .collect())
    }

    pub async fn infer_tags(&self, config: &Config, image_tensor_data: Vec<f32>) -> ApiResult<Vec<SmallString>> {
        let auto_tag_config = get_config(config)?;
        let mut session = self.session.lock().await;

        let _timer = crate::time::Timer::new("Inference");
        let image_tensor = Tensor::from_array(([1, 448, 448, 3], image_tensor_data)).map_err(ApiError::from)?;
        let outputs = session.run(inputs![self.input_name.clone() => image_tensor])?;
        let (_shape, confidences) = outputs[0].try_extract_tensor::<f32>()?;

        Ok(confidences
            .iter()
            .zip(&self.labels)
            .filter(|&(confidence, label)| match label.kind {
                LabelKind::Character => *confidence >= auto_tag_config.character_threshold,
                LabelKind::General => *confidence >= auto_tag_config.general_threshold,
                LabelKind::Rating => false,
            })
            .map(|(_, label)| SmallString::new(&label.name))
            .collect())
    }
}

#[derive(Default, FromRepr)]
enum LabelKind {
    Rating = 9,
    Character = 4,
    #[default]
    General = 0,
}

struct Label {
    name: String,
    kind: LabelKind,
}

fn get_config(config: &Config) -> ApiResult<&AutoTagConfig> {
    config
        .auto_tag
        .as_ref()
        .ok_or(ApiError::FeatureDisabled(OptionalFeature::AutoTag))
}

fn create_session(config: &Config) -> ApiResult<Session> {
    let auto_tag_config = get_config(config)?;
    let mut session_builder = Session::builder()?;

    // Try to register WebGPU and report any errors
    if let Err(err) = WebGPUExecutionProvider::default().register(&mut session_builder) {
        warn!("Failed to register WebGPU execution provider: {err}");
    }

    // CPU fallback (should always work)
    if let Err(err) = CPUExecutionProvider::default().register(&mut session_builder) {
        error!("Failed to register CPU execution provider: {err}");
    }

    let model_path = config.path(Directory::Models).join(&auto_tag_config.model);
    session_builder.commit_from_file(model_path).map_err(ApiError::from)
}

/// Load tags from CSV
fn load_labels(path: &Path) -> std::io::Result<Vec<Label>> {
    std::fs::read_to_string(path).map(|file_contents| {
        file_contents
            .lines()
            .skip(1)
            .filter_map(|line| {
                let mut column_iter = line.split(',').skip(1);
                if let (Some(name), Some(kind)) = (column_iter.next(), column_iter.next()) {
                    let name = name.to_owned();
                    let kind = kind.parse().ok().and_then(LabelKind::from_repr).unwrap_or_default();
                    Some(Label { name, kind })
                } else {
                    None
                }
            })
            .collect()
    })
}
