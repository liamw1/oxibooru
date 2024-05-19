use crate::math::cartesian::CartesianProduct;
use crate::math::func;
use crate::math::point::IPoint2;
use crate::math::rect::{Array2D, IRect};
use image::{DynamicImage, GrayImage};
use num_traits::ToPrimitive;
use std::cmp::{max, min};

pub fn compute_signature(image: &DynamicImage) -> Vec<i8> {
    let gray_image = image.to_luma8(); // Convert to 1:1 aspect ratio?
    let grid_points = compute_grid_points(&gray_image);
    let mean_matrix = compute_mean_matrix(&gray_image, &grid_points);
    let differentials = compute_differentials(&mean_matrix);
    normalize(&differentials, IDENTICAL_TOLERANCE)
}

pub fn normalized_distance(signature_a: &Vec<i8>, signature_b: &Vec<i8>) -> f64 {
    let l2_squared_distance = signature_a
        .iter()
        .zip(signature_b.iter())
        .map(|(&a, &b)| i64::from((a - b) * (a - b)))
        .sum::<i64>();
    let l2_distance = (l2_squared_distance as f64).sqrt();

    let l2_norm_a = (signature_a.iter().map(|a| i64::from(a * a)).sum::<i64>() as f64).sqrt();
    let l2_norm_b = (signature_b.iter().map(|b| i64::from(b * b)).sum::<i64>() as f64).sqrt();
    let denominator = l2_norm_a + l2_norm_b;

    if denominator == 0.0 {
        0.0
    } else {
        l2_distance / denominator
    }
}

const CROP_SCALE: f64 = 0.9;
const NUM_GRID_POINTS: usize = 9;
const FIXED_GRID_SQUARE_SIZE: Option<u32> = None;
const IDENTICAL_TOLERANCE: i16 = 2;

fn compute_grid_points(image: &GrayImage) -> CartesianProduct<u32, u32> {
    const LOWER_PERCENTILE: f64 = (1.0 - CROP_SCALE) / 2.0;
    let cropped_xmin = (LOWER_PERCENTILE * f64::from(image.width())).to_u32().unwrap();
    let cropped_ymin = (LOWER_PERCENTILE * f64::from(image.height())).to_u32().unwrap();
    let cropped_width = (CROP_SCALE * f64::from(image.width())).to_u32().unwrap();
    let cropped_height = (CROP_SCALE * f64::from(image.height())).to_u32().unwrap();

    let x_coords = func::linspace(cropped_xmin, cropped_xmin + cropped_width, NUM_GRID_POINTS);
    let y_coords = func::linspace(cropped_ymin, cropped_ymin + cropped_height, NUM_GRID_POINTS);
    CartesianProduct::new(x_coords, y_coords)
}

fn compute_mean_matrix(image: &GrayImage, grid_points: &CartesianProduct<u32, u32>) -> Array2D<u8> {
    let smallest_set_size = min(grid_points.left_set().len(), grid_points.right_set().len());
    let dynamic_grid_square_size = 0.5 + smallest_set_size as f64 / 20.0;
    let grid_square_size = FIXED_GRID_SQUARE_SIZE.unwrap_or(max(2, dynamic_grid_square_size.to_u32().unwrap()));

    let mut mean_matrix = Array2D::new_square(NUM_GRID_POINTS, 0).unwrap();
    for (matrix_index, (&pixel_i, &pixel_j)) in grid_points.index_iter().zip(grid_points.iter()) {
        let grid_square_center = IPoint2::new(pixel_i, pixel_j);
        let grid_square = IRect::new_centered_square(grid_square_center, grid_square_size / 2);
        let image_bounds = IRect::new_zero_based(image.dimensions().0, image.dimensions().1);
        let sum = IRect::intersection(grid_square, image_bounds)
            .iter()
            .map(|pixel_index| image.get_pixel(pixel_index.i, pixel_index.j))
            .map(|luma| u32::from(luma.0[0]))
            .sum::<u32>();

        let average = sum / grid_square.total_points().unwrap() as u32;
        mean_matrix.set_at(matrix_index, average.to_u8().unwrap());
    }
    mean_matrix
}

fn compute_differentials(mean_matrix: &Array2D<u8>) -> Vec<[i16; 8]> {
    mean_matrix
        .index_iter()
        .zip(mean_matrix.iter())
        .map(|(matrix_index, &center_value)| {
            IRect::new_centered_square(matrix_index, 1)
                .iter()
                .filter(|&neighbor| neighbor != matrix_index)
                .map(|neighbor| match mean_matrix.bounds().contains(neighbor) {
                    false => 0, // Difference assumed to be 0 if neighbor is outside grid bounds
                    true => i16::from(mean_matrix.at(neighbor)) - i16::from(center_value),
                })
                .collect::<Vec<_>>()
                .try_into()
                .expect("Expected exactly eight neighbors")
        })
        .collect()
}

fn compute_threshold<F: Fn(i16) -> bool>(differential_array: &Vec<[i16; 8]>, filter: F) -> Option<i16> {
    let mut filtered_values = differential_array
        .iter()
        .flat_map(|neighbors| neighbors.iter())
        .filter(|&&diff| filter(diff))
        .map(|&diff| diff)
        .collect::<Vec<_>>();
    filtered_values.sort();

    match filtered_values.len() {
        0 => None,
        n => Some(filtered_values[n / 2]),
    }
}

fn normalize(differentials: &Vec<[i16; 8]>, identical_tolerance: i16) -> Vec<i8> {
    let light_threshold = compute_threshold(differentials, |diff: i16| diff > identical_tolerance).unwrap_or(256);
    let dark_threshold = compute_threshold(differentials, |diff: i16| diff < identical_tolerance).unwrap_or(-256);

    differentials
        .iter()
        .flat_map(|neighbors| neighbors.iter())
        .map(|&diff| match diff {
            n if n < dark_threshold => -2,
            n if n < -identical_tolerance => -1,
            n if n < identical_tolerance => 0,
            n if n < light_threshold => 1,
            _ => 2,
        })
        .collect()
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::test::*;
    use std::path::Path;

    #[test]
    fn image_signature() {
        let image1 = image::open(asset_path(Path::new("jpeg.jpg"))).unwrap_or_else(|err| panic!("{err}"));
        let sig1 = compute_signature(&image1);

        let image2 = image::open(asset_path(Path::new("jpeg-similar.jpg"))).unwrap_or_else(|err| panic!("{err}"));
        let sig2 = compute_signature(&image2);

        assert!(normalized_distance(&sig1, &sig2) == 0.0);
    }
}
