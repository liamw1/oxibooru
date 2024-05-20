use crate::math::cartesian::CartesianProduct;
use crate::math::func;
use crate::math::point::IPoint2;
use crate::math::rect::{Array2D, IRect};
use image::{DynamicImage, GrayImage};
use num_traits::ToPrimitive;
use std::cmp::{max, min};

pub fn compute_signature(image: &DynamicImage) -> Vec<i8> {
    let gray_image = image.to_luma8();
    let grid_points = compute_grid_points(&gray_image);
    let mean_matrix = compute_mean_matrix(&gray_image, &grid_points);
    let differentials = compute_differentials(&mean_matrix);
    normalize(&differentials)
}

pub fn normalized_distance(signature_a: &Vec<i8>, signature_b: &Vec<i8>) -> f64 {
    let l2_squared_distance = signature_a
        .iter()
        .zip(signature_b.iter())
        .map(|(&a, &b)| (i64::from(a), i64::from(b)))
        .map(|(a, b)| (a - b) * (a - b))
        .sum::<i64>();
    let l2_distance = (l2_squared_distance as f64).sqrt();

    let l2_norm_a = (signature_a.iter().map(|&a| i64::from(a) * i64::from(a)).sum::<i64>() as f64).sqrt();
    let l2_norm_b = (signature_b.iter().map(|&b| i64::from(b) * i64::from(b)).sum::<i64>() as f64).sqrt();
    let denominator = l2_norm_a + l2_norm_b;

    if denominator == 0.0 {
        0.0
    } else {
        l2_distance / denominator
    }
}

const CROP_SCALE: f64 = 0.9;
const NUM_GRID_POINTS: usize = 9;
const FIXED_GRID_SQUARE_SIZE: Option<i32> = None;
const IDENTICAL_TOLERANCE: i16 = 2;
const N_LEVELS: usize = 2;

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
    let grid_square_size = FIXED_GRID_SQUARE_SIZE.unwrap_or(max(2, dynamic_grid_square_size.to_i32().unwrap()));
    let image_bounds = IRect::new_zero_based(image.dimensions().0 - 1, image.dimensions().1 - 1)
        .to_signed()
        .unwrap();

    let mut mean_matrix = Array2D::new_square(NUM_GRID_POINTS, 0);
    for (matrix_index, (&pixel_i, &pixel_j)) in grid_points.index_iter().zip(grid_points.iter()) {
        let grid_square_center = IPoint2::new(pixel_i, pixel_j).to_signed().unwrap();
        let grid_square = IRect::new_centered_square(grid_square_center, grid_square_size / 2);
        let sum = IRect::intersection(grid_square, image_bounds)
            .to_unsigned()
            .unwrap()
            .iter()
            .map(|pixel_index| image.get_pixel(pixel_index.i, pixel_index.j))
            .map(|luma| u64::from(luma.0[0]))
            .sum::<u64>();

        let average = sum / grid_square.total_points().unwrap();
        mean_matrix.set_at(matrix_index, average.to_u8().unwrap());
    }
    mean_matrix
}

fn compute_differentials(mean_matrix: &Array2D<u8>) -> Vec<[i16; 8]> {
    mean_matrix
        .index_iter()
        .map(|matrix_index| matrix_index.to_signed().unwrap())
        .zip(mean_matrix.iter())
        .map(|(matrix_index, &center_value)| {
            IRect::new_centered_square(matrix_index, 1)
                .iter()
                .filter(|&neighbor| neighbor != matrix_index)
                .map(|neighbor| match mean_matrix.get(neighbor) {
                    None => 0, // Difference assumed to be 0 if neighbor is outside grid bounds
                    Some(neighbor_value) => i16::from(neighbor_value) - i16::from(center_value),
                })
                .collect::<Vec<_>>()
                .try_into()
                .expect("Expected exactly eight neighbors")
        })
        .collect()
}

fn compute_cutoffs<F: Fn(i16) -> bool>(differentials: &Vec<[i16; 8]>, filter: F) -> Vec<Option<i16>> {
    let mut filtered_values = differentials
        .iter()
        .flat_map(|neighbors| neighbors.iter())
        .filter(|&&diff| filter(diff))
        .map(|&diff| diff)
        .collect::<Vec<_>>();
    filtered_values.sort();

    let chunk_size = match filtered_values.len() % N_LEVELS {
        0 => filtered_values.len() / N_LEVELS,
        _ => filtered_values.len() / N_LEVELS + 1,
    };

    filtered_values
        .chunks(chunk_size)
        .map(|chunk| chunk.last().map(|x| *x))
        .collect::<Vec<_>>()
}

fn normalize(differentials: &Vec<[i16; 8]>) -> Vec<i8> {
    let dark_cutoffs = compute_cutoffs(differentials, |diff: i16| diff < -IDENTICAL_TOLERANCE);
    let light_cutoffs = compute_cutoffs(differentials, |diff: i16| diff > IDENTICAL_TOLERANCE);

    let mut cutoffs = dark_cutoffs;
    cutoffs.push(Some(IDENTICAL_TOLERANCE));
    cutoffs.extend(light_cutoffs);
    assert_eq!(cutoffs.len(), 2 * N_LEVELS + 1);

    differentials
        .iter()
        .flat_map(|neighbors| neighbors.iter())
        .map(|&diff| {
            for (level, &cutoff) in cutoffs.iter().enumerate() {
                match cutoff {
                    Some(cutoff) if diff <= cutoff => return level as i8 - N_LEVELS as i8,
                    _ => (),
                }
            }
            panic!("Expected diff to be under at least one cutoff");
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
        let image1 = image::open(asset_path(Path::new("png.png"))).unwrap_or_else(|err| panic!("{err}"));
        let sig1 = compute_signature(&image1);
        assert_eq!(normalized_distance(&sig1, &sig1), 0.0);

        let image2 = image::open(asset_path(Path::new("bmp.bmp"))).unwrap_or_else(|err| panic!("{err}"));
        let sig2 = compute_signature(&image2);
        assert_eq!(normalized_distance(&sig2, &sig2), 0.0);

        let image3 = image::open(asset_path(Path::new("jpeg.jpg"))).unwrap_or_else(|err| panic!("{err}"));
        let sig3 = compute_signature(&image3);
        assert_eq!(normalized_distance(&sig3, &sig3), 0.0);

        let image4 = image::open(asset_path(Path::new("jpeg-similar.jpg"))).unwrap_or_else(|err| panic!("{err}"));
        let sig4 = compute_signature(&image4);
        assert_eq!(normalized_distance(&sig4, &sig4), 0.0);

        // Identical images of different formats
        assert_eq!(normalized_distance(&sig1, &sig2), 0.0);
        // Similar images of same format
        assert!(normalized_distance(&sig3, &sig4) - 0.18462541746035205 < 1e-8);
        // Different images
        assert!(normalized_distance(&sig1, &sig3) - 0.6441917374235336 < 1e-8);
    }
}
