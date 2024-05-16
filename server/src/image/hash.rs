use image::{GrayImage, ImageResult};
use std::cmp::{max, min};
use std::io::Cursor;

pub fn generate_signature(content: &[u8]) -> ImageResult<()> {
    let gray_image = preprocess_image(content)?;
    let (x_coords, y_coords) = compute_grid_points(&gray_image);

    Ok(())
}

const CROP_SCALE: f64 = 0.9;
const NUM_GRID_POINTS: usize = 9;
const FIXED_GRID_SQUARE_SIZE: Option<usize> = None;

fn linspace(start: usize, end: usize, num: usize) -> Vec<usize> {
    match num {
        0 => vec![],
        1 => {
            let midpoint = 0.5 * start as f64 + 0.5 * end as f64;
            vec![midpoint as usize]
        }
        n => {
            let step = (end - start) as f64 / (num - 1) as f64;
            (0..n)
                .map(|i| (start as f64 + i as f64 * step).round() as usize)
                .collect()
        }
    }
}

fn preprocess_image(content: &[u8]) -> ImageResult<GrayImage> {
    let decoded_image = image::load(Cursor::new(content), image::ImageFormat::Png)?;

    // Convert to 1:1 aspect ratio?

    Ok(decoded_image.to_luma8())
}

fn compute_grid_points(image: &GrayImage) -> (Vec<usize>, Vec<usize>) {
    const LOWER_PERCENTILE: f64 = (1.0 - CROP_SCALE) / 2.0;
    let cropped_xmin = LOWER_PERCENTILE * image.width() as f64;
    let cropped_ymin = LOWER_PERCENTILE * image.height() as f64;
    let cropped_width = CROP_SCALE * image.width() as f64;
    let cropped_height = CROP_SCALE * image.height() as f64;

    let x_coords = linspace(cropped_xmin as usize, (cropped_xmin + cropped_width) as usize, NUM_GRID_POINTS);
    let y_coords = linspace(cropped_ymin as usize, (cropped_ymin + cropped_height) as usize, NUM_GRID_POINTS);
    (x_coords, y_coords)
}

fn compute_mean_level(image: &GrayImage, x_coords: Vec<usize>, y_coords: Vec<usize>) {
    let dynamic_grid_square_size = 0.5 + min(x_coords.len(), y_coords.len()) as f64 / 20.0;
    let grid_square_size = FIXED_GRID_SQUARE_SIZE.unwrap_or(std::cmp::max(2, dynamic_grid_square_size as usize));
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn create_linspace() {
        assert_eq!(linspace(0, 2, 0), vec![]);
        assert_eq!(linspace(0, 2, 1), vec![1]);
        assert_eq!(linspace(0, 2, 2), vec![0, 2]);
        assert_eq!(linspace(0, 2, 3), vec![0, 1, 2]);
        assert_eq!(linspace(0, 5, 3), vec![0, 3, 5]);
        assert_eq!(linspace(0, 100, 6), vec![0, 20, 40, 60, 80, 100]);
        assert_eq!(linspace(0, 100, 7), vec![0, 17, 33, 50, 67, 83, 100]);
    }
}
