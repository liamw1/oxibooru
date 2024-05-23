use crate::math::cartesian::CartesianProduct;
use crate::math::func;
use crate::math::point::IPoint2;
use crate::math::rect::{Array2D, IRect};
use image::{DynamicImage, GrayImage};
use num_traits::ToPrimitive;

pub fn compute_signature(image: &DynamicImage) -> Vec<u8> {
    let gray_image = image.to_luma8();
    let grid_points = compute_grid_points(&gray_image);
    let mean_matrix = compute_mean_matrix(&gray_image, &grid_points);
    let differentials = compute_differentials(&mean_matrix);
    normalize(&differentials)
}

pub fn normalized_distance(signature_a: &Vec<u8>, signature_b: &Vec<u8>) -> f64 {
    let l2_squared_distance = signature_a
        .iter()
        .zip(signature_b.iter())
        .map(|(&a, &b)| (i64::from(a), i64::from(b)))
        .map(|(a, b)| (a - b) * (a - b))
        .sum::<i64>();
    let l2_distance = (l2_squared_distance as f64).sqrt();

    let offset = INTENSITY_LEVELS as i64;
    let l2_squared_norm_a: i64 = signature_a.iter().map(|&a| i64::from(a) - offset).map(|a| a * a).sum();
    let l2_squared_norm_b: i64 = signature_b.iter().map(|&b| i64::from(b) - offset).map(|b| b * b).sum();
    let denominator = (l2_squared_norm_a as f64).sqrt() + (l2_squared_norm_b as f64).sqrt();

    if denominator == 0.0 {
        0.0
    } else {
        l2_distance / denominator
    }
}

/*
    Implementation follows H. Chi Wong, Marshall Bern and David Goldberg with a few tweaks
*/

const CROP_PERCENTILE: u64 = 5;
const NUM_GRID_POINTS: u32 = 9;
const IDENTICAL_TOLERANCE: i16 = 2;
const INTENSITY_LEVELS: usize = 2;

fn grid_square_radius(width: u32, height: u32) -> u32 {
    let grid_square_size = 0.5 + std::cmp::min(width, height) as f64 / 20.0;
    (grid_square_size / 2.0).to_u32().unwrap()
}

fn compute_grid_points(image: &GrayImage) -> CartesianProduct<u32, u32> {
    let bounds = IRect::new_zero_based(image.width() - 2, image.height() - 2);

    let mut total_row_delta = 0;
    let mut total_column_delta = 0;
    for index in bounds.iter() {
        let pixel_value = image.get_pixel(index.i, index.j).0[0];

        let row_adjacent_value = image.get_pixel(index.i + 1, index.j).0[0];
        total_row_delta += u64::from(pixel_value.abs_diff(row_adjacent_value));

        let column_adjacent_value = image.get_pixel(index.i, index.j + 1).0[0];
        total_column_delta += u64::from(pixel_value.abs_diff(column_adjacent_value));
    }

    let row_delta_limit = CROP_PERCENTILE * total_row_delta / 100;
    let column_delta_limit = CROP_PERCENTILE * total_column_delta / 100;

    let mut lower_row_index = 0;
    let mut row_delta = 0;
    for i in 0..(image.width() - 1) {
        if row_delta >= row_delta_limit {
            lower_row_index = i;
            break;
        }

        for j in 0..image.height() {
            let pixel_value = image.get_pixel(i, j).0[0];
            let row_adjacent_value = image.get_pixel(i + 1, j).0[0];
            row_delta += u64::from(pixel_value.abs_diff(row_adjacent_value));
        }
    }

    let mut upper_row_index = image.width() - 1;
    let mut row_delta = 0;
    for i in (0..(image.width() - 1)).rev() {
        if row_delta >= row_delta_limit {
            upper_row_index = i;
            break;
        }

        for j in 0..image.height() {
            let pixel_value = image.get_pixel(i, j).0[0];
            let row_adjacent_value = image.get_pixel(i + 1, j).0[0];
            row_delta += u64::from(pixel_value.abs_diff(row_adjacent_value));
        }
    }

    let mut lower_column_index = 0;
    let mut column_delta = 0;
    for j in 0..(image.height() - 1) {
        if column_delta >= column_delta_limit {
            lower_column_index = j;
            break;
        }

        for i in 0..image.width() {
            let pixel_value = image.get_pixel(i, j).0[0];
            let column_adjacent_value = image.get_pixel(i, j + 1).0[0];
            column_delta += u64::from(pixel_value.abs_diff(column_adjacent_value));
        }
    }

    let mut upper_column_index = image.height() - 1;
    let mut column_delta = 0;
    for j in (0..(image.height() - 1)).rev() {
        if column_delta >= column_delta_limit {
            upper_column_index = j;
            break;
        }

        for i in 0..image.width() {
            let pixel_value = image.get_pixel(i, j).0[0];
            let column_adjacent_value = image.get_pixel(i, j + 1).0[0];
            column_delta += u64::from(pixel_value.abs_diff(column_adjacent_value));
        }
    }

    // Adjust cropped bounds so that grid squares won't protrude into image borders
    let estimated_grid_square_radius =
        grid_square_radius(upper_row_index - lower_row_index, upper_column_index - lower_column_index);
    lower_row_index += estimated_grid_square_radius;
    upper_row_index -= estimated_grid_square_radius;
    lower_column_index += estimated_grid_square_radius;
    upper_column_index -= estimated_grid_square_radius;

    let x_coords = func::symmetric_linspace(lower_row_index, upper_row_index, NUM_GRID_POINTS);
    let y_coords = func::symmetric_linspace(lower_column_index, upper_column_index, NUM_GRID_POINTS);

    CartesianProduct::new(x_coords, y_coords)
}

fn compute_mean_matrix(image: &GrayImage, grid_points: &CartesianProduct<u32, u32>) -> Array2D<u8> {
    let cropped_width = grid_points.left_set().last().unwrap() - grid_points.left_set().first().unwrap();
    let cropped_height = grid_points.right_set().last().unwrap() - grid_points.right_set().first().unwrap();
    let grid_square_radius = grid_square_radius(cropped_width, cropped_height) as i32;
    let image_bounds = IRect::new_zero_based(image.width() - 1, image.height() - 1)
        .to_signed()
        .unwrap();

    let mut mean_matrix = Array2D::new_square(NUM_GRID_POINTS, 0);
    for (matrix_index, (&pixel_i, &pixel_j)) in grid_points.enumerate() {
        let grid_square_center = IPoint2::new(pixel_i, pixel_j).to_signed().unwrap();
        let grid_square = IRect::new_centered_square(grid_square_center, grid_square_radius);
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

/*
    The original paper describes computing the differences of each grid square with its neighbors.
    Grids squares on the boundaries of the 9x9 grid will have no neighbors, so differences between them
    are considered 0. However, I would think that this would increase the likelihood of random
    signatures matching on certain words. It may be better to increase the size of the grid to 11x11
    and compute a signature using the interior 9x9 grid squares, which will all have neighbors.
    TODO: Test to see if this approach gets better discrimination on images.
*/
fn compute_differentials(mean_matrix: &Array2D<u8>) -> Vec<[i16; 8]> {
    //let center = NUM_GRID_POINTS / 2;
    //let bounds = IRect::new_centered_square(IPoint2::new(center, center), center - 1);
    mean_matrix
        .signed_enumerate()
        //.filter(|(matrix_index, _)| bounds.contains(*matrix_index))
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

    let chunk_size = match filtered_values.len() % INTENSITY_LEVELS {
        0 => filtered_values.len() / INTENSITY_LEVELS,
        _ => filtered_values.len() / INTENSITY_LEVELS + 1,
    };

    filtered_values
        .chunks(chunk_size)
        .map(|chunk| chunk.last().map(|x| *x))
        .collect::<Vec<_>>()
}

fn normalize(differentials: &Vec<[i16; 8]>) -> Vec<u8> {
    let dark_cutoffs = compute_cutoffs(differentials, |diff: i16| diff < -IDENTICAL_TOLERANCE);
    let light_cutoffs = compute_cutoffs(differentials, |diff: i16| diff > IDENTICAL_TOLERANCE);

    let mut cutoffs = dark_cutoffs;
    cutoffs.push(Some(IDENTICAL_TOLERANCE));
    cutoffs.extend(light_cutoffs);
    debug_assert_eq!(cutoffs.len(), 2 * INTENSITY_LEVELS + 1);

    differentials
        .iter()
        .flat_map(|neighbors| neighbors.iter())
        .map(|&diff| {
            for (level, &cutoff) in cutoffs.iter().enumerate() {
                match cutoff {
                    Some(cutoff) if diff <= cutoff => return level as u8,
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
    fn image_signature_regression() {
        let image1 = image::open(asset_path(Path::new("png.png"))).unwrap_or_else(|err| panic!("{err}"));
        let sig1 = compute_signature(&image1);

        let image2 = image::open(asset_path(Path::new("bmp.bmp"))).unwrap_or_else(|err| panic!("{err}"));
        let sig2 = compute_signature(&image2);

        let image3 = image::open(asset_path(Path::new("jpeg.jpg"))).unwrap_or_else(|err| panic!("{err}"));
        let sig3 = compute_signature(&image3);

        let image4 = image::open(asset_path(Path::new("jpeg-similar.jpg"))).unwrap_or_else(|err| panic!("{err}"));
        let sig4 = compute_signature(&image4);

        //assert_eq!(normalized_distance(&sig3, &sig4), 0.0);

        // Identical images of different formats
        assert_eq!(normalized_distance(&sig1, &sig2), 0.0);
        // Similar images of same format
        assert!((normalized_distance(&sig3, &sig4) - 0.14279835125009815).abs() < 1e-8);
        // Different images
        assert!((normalized_distance(&sig1, &sig3) - 0.5956693016498599).abs() < 1e-8);
    }

    #[test]
    fn signature_robustness() {
        let lisa = image::open(asset_path(Path::new("mona_lisa.jpg"))).unwrap_or_else(|err| panic!("{err}"));
        let lisa_signature = compute_signature(&lisa);

        let lisa_low_res =
            image::open(asset_path(Path::new("mona_lisa-low_res.jpg"))).unwrap_or_else(|err| panic!("{err}"));
        let lisa_low_res_signature = compute_signature(&lisa_low_res);

        let lisa_retouched =
            image::open(asset_path(Path::new("mona_lisa-retouched.jpg"))).unwrap_or_else(|err| panic!("{err}"));
        let lisa_retouched_signature = compute_signature(&lisa_retouched);

        let lisa_high_contrast =
            image::open(asset_path(Path::new("mona_lisa-high_contrast.jpg"))).unwrap_or_else(|err| panic!("{err}"));
        let lisa_high_contrast_signature = compute_signature(&lisa_high_contrast);

        let lisa_small_border =
            image::open(asset_path(Path::new("mona_lisa-small_border.jpg"))).unwrap_or_else(|err| panic!("{err}"));
        let lisa_small_border_signature = compute_signature(&lisa_small_border);

        let lisa_large_border =
            image::open(asset_path(Path::new("mona_lisa-large_border.jpg"))).unwrap_or_else(|err| panic!("{err}"));
        let lisa_large_border_signature = compute_signature(&lisa_large_border);

        //assert_eq!(normalized_distance(&lisa_signature, &lisa_retouched_signature), 0.0);

        assert!(normalized_distance(&lisa_signature, &lisa_low_res_signature) < 0.4);
        assert!(normalized_distance(&lisa_signature, &lisa_retouched_signature) < 0.45);
        assert!(normalized_distance(&lisa_signature, &lisa_high_contrast_signature) < 0.2);
        assert!(normalized_distance(&lisa_signature, &lisa_small_border_signature) < 0.5);
        assert!(normalized_distance(&lisa_signature, &lisa_large_border_signature) < 0.5);
    }

    #[test]
    fn grid_points() {
        let lisa_small_border = image::open(asset_path(Path::new("mona_lisa-small_border.jpg")))
            .unwrap_or_else(|err| panic!("{err}"))
            .to_luma8();

        let grid_points = compute_grid_points(&lisa_small_border);
        let lower_left_grid_point = (grid_points.left_set().first().unwrap(), grid_points.right_set().first().unwrap());
        let upper_right_grid_point = (grid_points.left_set().last().unwrap(), grid_points.right_set().last().unwrap());
        let lower_left_pixel = lisa_small_border.get_pixel(*lower_left_grid_point.0, *lower_left_grid_point.1);
        let upper_right_pixel = lisa_small_border.get_pixel(*upper_right_grid_point.0, *upper_right_grid_point.1);
        assert!(lower_left_pixel.0[0] < 250);
        assert!(upper_right_pixel.0[0] < 250);

        let lisa_large_border = image::open(asset_path(Path::new("mona_lisa-large_border.jpg")))
            .unwrap_or_else(|err| panic!("{err}"))
            .to_luma8();

        let grid_points = compute_grid_points(&lisa_large_border);
        let lower_left_grid_point = (grid_points.left_set().first().unwrap(), grid_points.right_set().first().unwrap());
        let upper_right_grid_point = (grid_points.left_set().last().unwrap(), grid_points.right_set().last().unwrap());
        let lower_left_pixel = lisa_large_border.get_pixel(*lower_left_grid_point.0, *lower_left_grid_point.1);
        let upper_right_pixel = lisa_large_border.get_pixel(*upper_right_grid_point.0, *upper_right_grid_point.1);
        assert!(lower_left_pixel.0[0] < 250);
        assert!(upper_right_pixel.0[0] < 250);
    }
}
