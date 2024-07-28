use crate::math::cartesian::CartesianProduct;
use crate::math::func;
use crate::math::interval::Interval;
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

pub fn normalized_distance(signature_a: &[u8], signature_b: &[u8]) -> f64 {
    let l2_squared_distance = signature_a
        .iter()
        .zip(signature_b.iter())
        .map(|(&a, &b)| (i64::from(a as i8), i64::from(b as i8)))
        .map(|(a, b)| (a - b) * (a - b))
        .sum::<i64>();
    let l2_distance = (l2_squared_distance as f64).sqrt();

    let l2_squared_norm_a: i64 = signature_a.iter().map(|&a| i64::from(a as i8)).map(|a| a * a).sum();
    let l2_squared_norm_b: i64 = signature_b.iter().map(|&b| i64::from(b as i8)).map(|b| b * b).sum();
    let denominator = (l2_squared_norm_a as f64).sqrt() + (l2_squared_norm_b as f64).sqrt();

    if denominator == 0.0 {
        0.0
    } else {
        l2_distance / denominator
    }
}

pub fn generate_indexes(signature: &[u8]) -> Vec<i32> {
    const NUM_REDUCED_SYMBOLS: i32 = 3;
    debug_assert!(i64::from(NUM_REDUCED_SYMBOLS).pow(NUM_LETTERS) < i64::from(i32::MAX));

    const NUM_LETTERS_USIZE: usize = NUM_LETTERS as usize;
    let word_positions = func::linspace(0, signature.len() - NUM_LETTERS_USIZE, NUM_WORDS);

    let words: Vec<[u8; NUM_LETTERS_USIZE]> = word_positions
        .iter()
        .map(|&pos| signature[pos..(pos + NUM_LETTERS_USIZE)].try_into().unwrap())
        .collect();

    words
        .iter()
        .map(|word| {
            word.iter()
                .map(|&letter| letter as i8)
                .map(|letter| letter.clamp(-1, 1))
                .enumerate()
                .map(|(i, letter)| i32::from(letter + 1) * NUM_REDUCED_SYMBOLS.pow(i as u32))
                .sum()
        })
        .collect()
}

/*
    Implementation follows H. Chi Wong, Marshall Bern and David Goldberg with a few tweaks
*/

const CROP_PERCENTILE: u64 = 5;
const NUM_GRID_POINTS: u32 = 9;
const IDENTICAL_TOLERANCE: i16 = 1;
const LUMINANCE_LEVELS: u32 = 2;
const NUM_WORDS: u32 = 100;
const NUM_LETTERS: u32 = 16;
const NUM_SYMBOLS: usize = 2 * LUMINANCE_LEVELS as usize + 1;

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

    let calc_row_delta = |i: u32| -> u64 {
        (0..image.height())
            .into_iter()
            .map(|j| {
                let pixel_value = image.get_pixel(i, j).0[0];
                let row_adjacent_value = image.get_pixel(i + 1, j).0[0];
                u64::from(pixel_value.abs_diff(row_adjacent_value))
            })
            .sum()
    };
    let calc_column_delta = |j: u32| -> u64 {
        (0..image.width())
            .into_iter()
            .map(|i| {
                let pixel_value = image.get_pixel(i, j).0[0];
                let column_adjacent_value = image.get_pixel(i, j + 1).0[0];
                u64::from(pixel_value.abs_diff(column_adjacent_value))
            })
            .sum()
    };

    let row_delta_limit = CROP_PERCENTILE * total_row_delta / 100;
    let column_delta_limit = CROP_PERCENTILE * total_column_delta / 100;

    let rows = Interval::new(0, image.width() - 2);
    let columns = Interval::new(0, image.height() - 2);

    let mut lower_row_index = 0;
    let mut cumulative_row_delta = 0;
    for i in rows.iter() {
        if cumulative_row_delta >= row_delta_limit {
            lower_row_index = i;
            break;
        }
        cumulative_row_delta += calc_row_delta(i);
    }

    let mut upper_row_index = image.width() - 1;
    let mut cumulative_row_delta = 0;
    for i in rows.iter().rev() {
        if cumulative_row_delta >= row_delta_limit {
            upper_row_index = i;
            break;
        }
        cumulative_row_delta += calc_row_delta(i);
    }

    let mut lower_column_index = 0;
    let mut cumulative_column_delta = 0;
    for j in columns.iter() {
        if cumulative_column_delta >= column_delta_limit {
            lower_column_index = j;
            break;
        }
        cumulative_column_delta += calc_column_delta(j);
    }

    let mut upper_column_index = image.height() - 1;
    let mut cumulative_column_delta = 0;
    for j in columns.iter().rev() {
        if cumulative_column_delta >= column_delta_limit {
            upper_column_index = j;
            break;
        }
        cumulative_column_delta += calc_column_delta(j);
    }

    // Adjust cropped bounds so that grid squares won't protrude into image borders
    let estimated_grid_square_radius =
        grid_square_radius(upper_row_index - lower_row_index, upper_column_index - lower_column_index);
    lower_row_index += estimated_grid_square_radius;
    upper_row_index -= estimated_grid_square_radius;
    lower_column_index += estimated_grid_square_radius;
    upper_column_index -= estimated_grid_square_radius;

    let x_coords = func::linspace(lower_row_index, upper_row_index, NUM_GRID_POINTS);
    let y_coords = func::linspace(lower_column_index, upper_column_index, NUM_GRID_POINTS);

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
    signatures matching on certain words. I've simply excluded these differences from the final signature.
*/
fn compute_differentials(mean_matrix: &Array2D<u8>) -> Vec<i16> {
    mean_matrix
        .signed_enumerate()
        .flat_map(|(matrix_index, &center_value)| {
            IRect::new_centered_square(matrix_index, 1)
                .iter()
                .filter(move |&neighbor| neighbor != matrix_index)
                .filter_map(move |neighbor| {
                    mean_matrix
                        .get(neighbor)
                        .map(|neighbor_value| i16::from(neighbor_value) - i16::from(center_value))
                })
        })
        .collect()
}

fn compute_cutoffs<F: Fn(i16) -> bool>(differentials: &[i16], filter: F) -> Vec<Option<i16>> {
    let mut filtered_values = differentials
        .iter()
        .copied()
        .filter(|&diff| filter(diff))
        .collect::<Vec<_>>();
    filtered_values.sort();

    const LUMINANCE_LEVELS_USIZE: usize = LUMINANCE_LEVELS as usize;
    let chunk_size = match filtered_values.len() % LUMINANCE_LEVELS_USIZE {
        0 => filtered_values.len() / LUMINANCE_LEVELS_USIZE,
        _ => filtered_values.len() / LUMINANCE_LEVELS_USIZE + 1,
    };

    filtered_values
        .chunks(chunk_size)
        .map(|chunk| chunk.last().copied())
        .collect()
}

fn normalize(differentials: &[i16]) -> Vec<u8> {
    let dark_cutoffs = compute_cutoffs(differentials, |diff: i16| diff < -IDENTICAL_TOLERANCE);
    let light_cutoffs = compute_cutoffs(differentials, |diff: i16| diff > IDENTICAL_TOLERANCE);

    let cutoffs = dark_cutoffs
        .into_iter()
        .chain(std::iter::once(Some(IDENTICAL_TOLERANCE)))
        .chain(light_cutoffs.into_iter())
        .collect::<Vec<_>>();
    debug_assert_eq!(cutoffs.len(), NUM_SYMBOLS);

    differentials
        .iter()
        .map(|&diff| {
            cutoffs
                .iter()
                .position(|opt_cutoff| opt_cutoff.map(|cutoff| diff <= cutoff).unwrap_or(false))
                .expect("Expected diff to be under at least one cutoff") as i8
        })
        .map(|level| level - LUMINANCE_LEVELS as i8) // Map to range of [-LUMINANCE_LEVELS, LUMINANCE_LEVELS]
        .map(|level| level as u8) // Convert to byte. Can convert back by casting back to i8
        .collect()
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::test::*;
    use std::path::Path;

    #[test]
    fn image_signature_regression() {
        let image1 = image::open(asset_path(Path::new("png.png"))).unwrap();
        let sig1 = compute_signature(&image1);

        let image2 = image::open(asset_path(Path::new("bmp.bmp"))).unwrap();
        let sig2 = compute_signature(&image2);

        let image3 = image::open(asset_path(Path::new("jpeg.jpg"))).unwrap();
        let sig3 = compute_signature(&image3);

        let image4 = image::open(asset_path(Path::new("jpeg-similar.jpg"))).unwrap();
        let sig4 = compute_signature(&image4);

        // println!("");
        // println!("Distances:");
        // println!("{}", normalized_distance(&sig3, &sig4));
        // println!("{}", normalized_distance(&sig1, &sig3));
        // println!("");

        // Identical images of different formats
        assert_eq!(normalized_distance(&sig1, &sig2), 0.0);
        // Similar images of same format
        assert!((normalized_distance(&sig3, &sig4) - 0.1583484677615785).abs() < 1e-8);
        // Different images
        assert!((normalized_distance(&sig1, &sig3) - 0.6990083687106061).abs() < 1e-8);
    }

    #[test]
    fn signature_robustness() {
        let lisa = image::open(asset_path(Path::new("lisa.jpg"))).unwrap();
        let lisa_signature = compute_signature(&lisa);
        let lisa_indexes = generate_indexes(&lisa_signature);

        let lisa_border = image::open(asset_path(Path::new("lisa-border.jpg"))).unwrap();
        let lisa_border_signature = compute_signature(&lisa_border);
        let lisa_border_indexes = generate_indexes(&lisa_border_signature);

        let lisa_large_border = image::open(asset_path(Path::new("lisa-large_border.jpg"))).unwrap();
        let lisa_large_border_signature = compute_signature(&lisa_large_border);
        let lisa_large_border_indexes = generate_indexes(&lisa_large_border_signature);

        let lisa_wide = image::open(asset_path(Path::new("lisa-wide.jpg"))).unwrap();
        let lisa_wide_signature = compute_signature(&lisa_wide);
        let lisa_wide_indexes = generate_indexes(&lisa_wide_signature);

        let lisa_cat = image::open(asset_path(Path::new("lisa-cat.jpg"))).unwrap();
        let lisa_cat_signature = compute_signature(&lisa_cat);
        let lisa_cat_indexes = generate_indexes(&lisa_cat_signature);

        let starry_night = image::open(asset_path(Path::new("starry_night.jpg"))).unwrap();
        let starry_night_signature = compute_signature(&starry_night);
        let starry_night_indexes = generate_indexes(&starry_night_signature);

        // println!("");
        // println!("Distances:");
        // println!("{}", normalized_distance(&lisa_signature, &lisa_border_signature));
        // println!("{}", normalized_distance(&lisa_signature, &lisa_large_border_signature));
        // println!("{}", normalized_distance(&lisa_signature, &lisa_wide_signature));
        // println!("{}", normalized_distance(&lisa_signature, &lisa_cat_signature));
        // println!("{}", normalized_distance(&lisa_signature, &starry_night_signature));
        // println!("Matches:");
        // println!("{}", matching_indexes(&lisa_indexes, &lisa_border_indexes));
        // println!("{}", matching_indexes(&lisa_indexes, &lisa_large_border_indexes));
        // println!("{}", matching_indexes(&lisa_indexes, &lisa_wide_indexes));
        // println!("{}", matching_indexes(&lisa_indexes, &lisa_cat_indexes));
        // println!("{}", matching_indexes(&lisa_indexes, &starry_night_indexes));
        // println!("");

        assert!(normalized_distance(&lisa_signature, &lisa_border_signature) < 0.2);
        assert!(normalized_distance(&lisa_signature, &lisa_large_border_signature) < 0.2);
        assert!(normalized_distance(&lisa_signature, &lisa_wide_signature) < 0.3);
        assert!(normalized_distance(&lisa_signature, &lisa_cat_signature) < 0.55);

        assert!(matching_indexes(&lisa_indexes, &lisa_border_indexes) > 0);
        assert!(matching_indexes(&lisa_indexes, &lisa_large_border_indexes) > 0);
        assert!(matching_indexes(&lisa_indexes, &lisa_wide_indexes) > 0);
        assert!(matching_indexes(&lisa_indexes, &lisa_cat_indexes) > 0);
        assert_eq!(matching_indexes(&lisa_indexes, &starry_night_indexes), 0);
    }

    #[test]
    fn grid_points() {
        let lisa_small_border = image::open(asset_path(Path::new("lisa-border.jpg")))
            .unwrap()
            .to_luma8();

        let grid_points = compute_grid_points(&lisa_small_border);
        let lower_left_grid_point = (grid_points.left_set().first().unwrap(), grid_points.right_set().first().unwrap());
        let upper_right_grid_point = (grid_points.left_set().last().unwrap(), grid_points.right_set().last().unwrap());
        let lower_left_pixel = lisa_small_border.get_pixel(*lower_left_grid_point.0, *lower_left_grid_point.1);
        let upper_right_pixel = lisa_small_border.get_pixel(*upper_right_grid_point.0, *upper_right_grid_point.1);
        assert!(lower_left_pixel.0[0] < 250);
        assert!(upper_right_pixel.0[0] < 250);

        let lisa_large_border = image::open(asset_path(Path::new("lisa-large_border.jpg")))
            .unwrap()
            .to_luma8();

        let grid_points = compute_grid_points(&lisa_large_border);
        let lower_left_grid_point = (grid_points.left_set().first().unwrap(), grid_points.right_set().first().unwrap());
        let upper_right_grid_point = (grid_points.left_set().last().unwrap(), grid_points.right_set().last().unwrap());
        let lower_left_pixel = lisa_large_border.get_pixel(*lower_left_grid_point.0, *lower_left_grid_point.1);
        let upper_right_pixel = lisa_large_border.get_pixel(*upper_right_grid_point.0, *upper_right_grid_point.1);
        assert!(lower_left_pixel.0[0] < 250);
        assert!(upper_right_pixel.0[0] < 250);
    }

    fn matching_indexes(indexes_a: &[i32], indexes_b: &[i32]) -> usize {
        indexes_a.iter().zip(indexes_b.iter()).filter(|(a, b)| a == b).count()
    }
}
