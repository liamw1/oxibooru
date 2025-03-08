use crate::math::cartesian::CartesianProduct;
use crate::math::interval::Interval;
use crate::math::point::IPoint2;
use crate::math::rect::{Array2D, IRect};
use image::{DynamicImage, GrayImage};
use num_traits::ToPrimitive;
use std::num::NonZeroU64;

pub const NUM_WORDS: usize = 100; // Number indexes to create from signature
pub const COMPRESSED_SIGNATURE_LEN: usize = SIGNATURE_LEN.div_ceil(SIGNATURE_DIGITS);
pub const SIGNATURE_VERSION: i32 = 1; // Bump this whenever post signatures change

pub struct Cache {
    signature: [u8; SIGNATURE_LEN],
    norm: f64,
}

/// Calculates a compact "signature" for an image that can be used for similarity search.
///
/// Implementation follows H. Chi Wong, Marshall Bern and David Goldberg with a few tweaks
pub fn compute(image: &DynamicImage) -> [i64; COMPRESSED_SIGNATURE_LEN] {
    let gray_image = image.to_luma8();
    let (grid_points, grid_square_radius) = compute_grid_points(&gray_image);
    let mean_matrix = compute_intensity_matrix(&gray_image, &grid_points, grid_square_radius as i32);
    let differences = compute_differences(&mean_matrix);
    let signature = normalize(&differences);
    compress(&signature)
}

pub fn cache(compressed_signature: &[i64; COMPRESSED_SIGNATURE_LEN]) -> Cache {
    let signature = uncompress(compressed_signature);
    Cache {
        signature,
        norm: norm(&signature),
    }
}

/// Computes a "distance" between two images based on their signatures.
/// Result is a number in the interval \[0, 1\].
///
/// The lower the number, the more similar the two images are.
pub fn distance(signature_a_cache: &Cache, signature_b_compressed: &[i64; COMPRESSED_SIGNATURE_LEN]) -> f64 {
    let signature_b = uncompress(signature_b_compressed);
    let l2_squared_distance: i64 = signature_a_cache
        .signature
        .into_iter()
        .zip(signature_b)
        .map(|(a, b)| (i64::from(a), i64::from(b)))
        .map(|(a, b)| (a - b) * (a - b))
        .sum();
    let l2_distance = (l2_squared_distance as f64).sqrt();

    let denominator = signature_a_cache.norm + norm(&signature_b);
    if denominator == 0.0 {
        0.0
    } else {
        l2_distance / denominator
    }
}

/// Creates a set of indices from an image signature.
/// The signature is divided into a set intervals called words, which are allowed to overlap.
/// The "letters" of each word are values in the image signature, clamped between \[-1, 1\].
/// Therefore, each word can be represented by a number in base-3, which we encode into an u32
/// (which we then convert to an i32). The highest N trits of the u32 are reserved for storing
/// the word index, where N is the number of trits required to store NUM_WORDS. This is so we
/// can use the `&&` array operator in Postgres to find matching indexes quickly.
pub fn generate_indexes(compressed_signature: &[i64; COMPRESSED_SIGNATURE_LEN]) -> [i32; NUM_WORDS] {
    const NUM_REDUCED_SYMBOLS: u32 = 3;
    const _: () = assert!(NUM_REDUCED_SYMBOLS % 2 == 1); // Number of reduced symbols must be odd
    const NUM_WORD_DIGITS: u32 = NUM_WORDS.ilog(NUM_REDUCED_SYMBOLS as usize) + 1; // Number of trits it takes to store NUM_WORDS
    const _: () = assert!(NUM_LETTERS as u32 + NUM_WORD_DIGITS <= u32::MAX.ilog(NUM_REDUCED_SYMBOLS)); // Make sure that information needed can't exceed u32 trits

    let signature = uncompress(compressed_signature);
    let word_positions: [usize; NUM_WORDS] = Interval::new(0, signature.len() - NUM_LETTERS).linspace();
    let words: [[u8; NUM_LETTERS]; NUM_WORDS] = std::array::from_fn(|word_index| {
        let pos = word_positions[word_index];
        signature[pos..(pos + NUM_LETTERS)].try_into().unwrap()
    });

    const CLAMP_VALUE: i8 = NUM_REDUCED_SYMBOLS as i8 / 2;
    std::array::from_fn(|word_index| {
        let word = words[word_index];
        let encoded_letters: u32 = word
            .iter()
            .map(|&letter| letter as i8 - LUMINANCE_LEVELS as i8)
            .map(|letter| letter.clamp(-CLAMP_VALUE, CLAMP_VALUE))
            .enumerate()
            .map(|(letter_index, letter)| (letter + CLAMP_VALUE) as u32 * NUM_REDUCED_SYMBOLS.pow(letter_index as u32))
            .sum();
        (word_index as u32 + NUM_REDUCED_SYMBOLS.pow(NUM_WORD_DIGITS) * encoded_letters) as i32
    })
}

const CROP_PERCENTILE: u64 = 1;
const GRID_SIZE: usize = 9; // Size of the square grid used to compute signature
const IDENTICAL_TOLERANCE: i16 = 1; // Pixel intensities within this distance will be treated as identical
const LUMINANCE_LEVELS: usize = 2; // How many shades of light/dark
const NUM_LETTERS: usize = 12; // Length of each index
const NUM_SYMBOLS: usize = 2 * LUMINANCE_LEVELS + 1; // Number of possible values letters can take
const SIGNATURE_LEN: usize = 8 * (GRID_SIZE - 2).pow(2) + 20 * (GRID_SIZE - 2) + 12;
const SIGNATURE_DIGITS: usize = u64::MAX.ilog(NUM_SYMBOLS as u64) as usize - 1;

type GridPoints = CartesianProduct<u32, u32, GRID_SIZE, GRID_SIZE>;

/// Creates an iterator of length `N` from give iterator `iter`.
/// Panics if the `iter` does not contain `N` elements.
fn array_from_iter<I, T, const N: usize>(iter: I) -> [T; N]
where
    I: Iterator<Item = T>,
    T: Copy + Default,
{
    let mut array = [T::default(); N];

    let mut iter_len = 0; // This should be optimized away in release builds
    for (array_value, iter_item) in array.iter_mut().zip(iter) {
        *array_value = iter_item;
        iter_len += 1;
    }
    debug_assert_eq!(iter_len, N);

    array
}

fn crop_position(iter: impl Iterator<Item = u64>, total_delta: u64) -> usize {
    let delta_limit = CROP_PERCENTILE * total_delta / 100;
    let iter = std::iter::once(0).chain(iter);
    iter.scan(0, |cumulative_delta, delta| {
        *cumulative_delta += delta;
        Some(*cumulative_delta)
    })
    .position(|cumulative_delta| cumulative_delta >= delta_limit)
    .unwrap_or(0)
}

fn crop(deltas: &[u64]) -> Interval<u32> {
    let total_delta = deltas.iter().sum();
    let lower = crop_position(deltas.iter().copied(), total_delta);
    let upper = deltas.len().saturating_sub(1) - crop_position(deltas.iter().rev().copied(), total_delta);
    Interval::new(lower as u32, upper as u32)
}

/// Computes sum of absolute differences of pixel intensities for neighboring rows.
fn compute_row_deltas(image: &GrayImage) -> Vec<u64> {
    let image_width = image.width() as usize;
    let flat_samples = image.as_flat_samples();
    (0..image.height() as usize - 1)
        .map(|j| {
            let row_start = j * image_width;
            let next_row_start = (j + 1) * image_width;
            let row = &flat_samples.as_slice()[row_start..row_start + image_width];
            let next_row = &flat_samples.as_slice()[next_row_start..next_row_start + image_width];

            row.iter()
                .zip(next_row)
                .map(|(pixel, &next_pixel)| u64::from(pixel.abs_diff(next_pixel)))
                .sum()
        })
        .collect()
}

/// Computes sum of absolute differences of pixel pixel intensities for neighboring columns.
/// The inner loop iterates over rows instead of columns for better memory access patterns.
fn compute_column_deltas(image: &GrayImage) -> Vec<u64> {
    let image_width = image.width() as usize;
    let mut deltas = vec![0; image_width - 1];

    let flat_samples = image.as_flat_samples();
    for j in 0..image.height() as usize {
        let row_start = j * image_width;
        let row = &flat_samples.as_slice()[row_start..row_start + image_width];

        for (delta, pixels) in deltas.iter_mut().zip(row.windows(2)) {
            let [pixel, next_pixel] = pixels.try_into().unwrap();
            *delta += u64::from(pixel.abs_diff(next_pixel));
        }
    }
    deltas
}

/// Determines where how the grid points should be placed on the given `image`.
/// Returns the positions of the grid points and their size.
fn compute_grid_points(image: &GrayImage) -> (GridPoints, u32) {
    let row_deltas = compute_row_deltas(image);
    let column_deltas = compute_column_deltas(image);
    let mut cropped_x_bounds = crop(&column_deltas);
    let mut cropped_y_bounds = crop(&row_deltas);

    // Compute grid square radius
    let grid_square_size = 0.5 + std::cmp::min(cropped_x_bounds.length(), cropped_y_bounds.length()) as f64 / 20.0;
    let grid_square_radius = (grid_square_size / 2.0).to_u32().unwrap();

    // Adjust cropped bounds so that grid squares won't protrude into image borders
    cropped_x_bounds.shrink(grid_square_radius);
    cropped_y_bounds.shrink(grid_square_radius);

    let x_coords: [u32; GRID_SIZE] = cropped_x_bounds.linspace();
    let y_coords: [u32; GRID_SIZE] = cropped_y_bounds.linspace();
    (CartesianProduct::new(x_coords, y_coords), grid_square_radius)
}

/// For a given `image`, a set of `grid_points`, and a `grid_square_radius`,
/// computes the average intensity of pixels within each grid point.
fn compute_intensity_matrix(
    image: &GrayImage,
    grid_points: &GridPoints,
    grid_square_radius: i32,
) -> Array2D<u8, GRID_SIZE, GRID_SIZE> {
    let image_bounds = IRect::new_zero_based(image.width() - 1, image.height() - 1)
        .to_signed()
        .unwrap();

    let mut intensity_matrix = Array2D::new(0);
    for (matrix_index, (&pixel_i, &pixel_j)) in grid_points.indexed_iter() {
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
        intensity_matrix.set_at(matrix_index, average.to_u8().unwrap());
    }
    intensity_matrix
}

/// The original paper describes computing the differences of each grid square with its neighbors.
/// Grids squares on the boundaries of the 9x9 grid will have no neighbors, so differences between them
/// are considered 0. However, I would think that this would increase the likelihood of random
/// signatures matching on certain words. I've simply excluded these differences from the final signature.
fn compute_differences(intensity_matrix: &Array2D<u8, GRID_SIZE, GRID_SIZE>) -> [i16; SIGNATURE_LEN] {
    let difference_iter = intensity_matrix
        .signed_indexed_iter()
        .flat_map(|(matrix_index, &center_value)| {
            IRect::new_centered_square(matrix_index, 1)
                .iter()
                .filter(move |&neighbor| neighbor != matrix_index)
                .filter_map(move |neighbor| {
                    intensity_matrix
                        .get(neighbor)
                        .map(|neighbor_value| i16::from(neighbor_value) - i16::from(center_value))
                })
        });
    array_from_iter(difference_iter)
}

/// Determines the pixel intensity values that define a transition thresholds between luminance levels.
/// Each luminance level should contain a roughly equal number of intensity values.
fn compute_cutoffs<F: Fn(i16) -> bool>(
    differences: &[i16; SIGNATURE_LEN],
    filter: F,
) -> [Option<i16>; LUMINANCE_LEVELS] {
    let mut filtered_values = differences
        .iter()
        .copied()
        .filter(|&diff| filter(diff))
        .collect::<Vec<_>>();
    filtered_values.sort();

    let chunk_size = match filtered_values.len() % LUMINANCE_LEVELS {
        0 => filtered_values.len() / LUMINANCE_LEVELS,
        _ => filtered_values.len() / LUMINANCE_LEVELS + 1,
    }
    .max(1); // Make sure chunk_size is not 0

    // Cutoff array may need to be padded if there are not enough filtered values to split into LUMINANCE_LEVELS chunks
    let pad_amount = LUMINANCE_LEVELS.saturating_sub(filtered_values.len());
    let chunks = filtered_values.chunks(chunk_size).map(|chunk| chunk.last().copied());
    array_from_iter(std::iter::repeat(None).take(pad_amount).chain(chunks))
}

/// The final stage of image signature generation. Maps each value in `differences` to the
/// interval \[0, NUM_SYMBOLS\] based on their sign and relative magnitude.
fn normalize(differences: &[i16; SIGNATURE_LEN]) -> [u8; SIGNATURE_LEN] {
    let dark_cutoffs = compute_cutoffs(differences, |diff: i16| diff < -IDENTICAL_TOLERANCE);
    let light_cutoffs = compute_cutoffs(differences, |diff: i16| diff > IDENTICAL_TOLERANCE);
    let cutoffs: [Option<i16>; NUM_SYMBOLS] = array_from_iter(
        dark_cutoffs
            .into_iter()
            .chain(std::iter::once(Some(IDENTICAL_TOLERANCE)))
            .chain(light_cutoffs),
    );

    let signature_iter = differences.iter().map(|&diff| {
        cutoffs
            .iter()
            .position(|opt_cutoff| opt_cutoff.map(|cutoff| diff <= cutoff).unwrap_or(false))
            .expect("Expected diff to be under at least one cutoff") as u8
    });
    array_from_iter(signature_iter)
}

fn compress(uncompressed_signature: &[u8; SIGNATURE_LEN]) -> [i64; COMPRESSED_SIGNATURE_LEN] {
    let compression_iter = uncompressed_signature.chunks(SIGNATURE_DIGITS).map(|chunk| {
        chunk
            .iter()
            .enumerate()
            .map(|(letter_index, &letter)| u64::from(letter) * NUM_SYMBOLS.pow(letter_index as u32) as u64)
            .sum::<u64>() as i64
    });
    array_from_iter(compression_iter)
}

fn uncompress(compressed_signature: &[i64; COMPRESSED_SIGNATURE_LEN]) -> [u8; SIGNATURE_LEN] {
    // Create divisor as NonZeroU64 to guarantee compiler doesn't generate divide-by-zero check
    const DIVISOR: NonZeroU64 = NonZeroU64::new(NUM_SYMBOLS as u64).unwrap();
    let decompression_iter = compressed_signature.iter().map(|&sum| sum as u64).flat_map(|mut sum| {
        (0..SIGNATURE_DIGITS).map(move |_| {
            let letter = sum % DIVISOR;
            sum /= DIVISOR;
            letter as u8
        })
    });
    array_from_iter(decompression_iter)
}

fn norm(signature: &[u8; SIGNATURE_LEN]) -> f64 {
    let norm_squard: i64 = signature
        .iter()
        .map(|&a| i64::from(a) - LUMINANCE_LEVELS as i64)
        .map(|a| a * a)
        .sum();
    (norm_squard as f64).sqrt()
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::test::*;
    use image::ImageResult;

    #[test]
    fn edge_cases() -> ImageResult<()> {
        let (one_pixel, _) = image_properties("1_pixel.png")?;
        let (monochrome, _) = image_properties("monochrome.png")?;
        let (gradient, _) = image_properties("gradient.png")?;

        let one_pixel_signature_cache = cache(&one_pixel);
        assert_eq!(distance(&one_pixel_signature_cache, &monochrome), 0.0);
        assert_ne!(distance(&one_pixel_signature_cache, &gradient), 0.0);
        Ok(())
    }

    #[test]
    fn image_signature_regression() -> ImageResult<()> {
        let (png, _) = image_properties("png.png")?;
        let (bmp, _) = image_properties("bmp.bmp")?;
        let (jpeg, _) = image_properties("jpeg.jpg")?;
        let (similar_jpeg, _) = image_properties("jpeg-similar.jpg")?;

        // Identical images of different formats
        assert_eq!(distance(&cache(&png), &bmp), 0.0);
        // Similar images of same format
        assert!((distance(&cache(&jpeg), &similar_jpeg) - 0.24372455010006977).abs() < 1e-8);
        // Different images
        assert!((distance(&cache(&png), &jpeg) - 0.7062433297659159).abs() < 1e-8);
        Ok(())
    }

    #[test]
    fn signature_robustness() -> ImageResult<()> {
        let (lisa_signature, lisa_indexes) = image_properties("lisa.jpg")?;
        let (lisa_border_signature, lisa_border_indexes) = image_properties("lisa-border.jpg")?;
        let (lisa_large_border_signature, lisa_large_border_indexes) = image_properties("lisa-large_border.jpg")?;
        let (lisa_wide_signature, lisa_wide_indexes) = image_properties("lisa-wide.jpg")?;
        let (lisa_cat_signature, lisa_cat_indexes) = image_properties("lisa-cat.jpg")?;
        let (starry_night_signature, _starry_night_indexes) = image_properties("starry_night.jpg")?;

        let lisa_signature_cache = cache(&lisa_signature);
        assert!(distance(&lisa_signature_cache, &lisa_border_signature) < 0.2);
        assert!(distance(&lisa_signature_cache, &lisa_large_border_signature) < 0.2);
        assert!(distance(&lisa_signature_cache, &lisa_wide_signature) < 0.25);
        assert!(distance(&lisa_signature_cache, &lisa_cat_signature) < 0.45);
        assert!(distance(&lisa_signature_cache, &starry_night_signature) > 0.6);

        assert!(matching_indexes(&lisa_indexes, &lisa_border_indexes) > 0);
        assert!(matching_indexes(&lisa_indexes, &lisa_large_border_indexes) > 0);
        assert!(matching_indexes(&lisa_indexes, &lisa_wide_indexes) > 0);
        assert!(matching_indexes(&lisa_indexes, &lisa_cat_indexes) > 0);
        // Starry night indexes are not checked here because indexes can match on pure chance
        Ok(())
    }

    #[test]
    fn grid_points() -> ImageResult<()> {
        let lisa_small_border = image::open(image_path("lisa-border.jpg"))?.to_luma8();

        let (grid_points, _) = compute_grid_points(&lisa_small_border);
        let lower_left_grid_point = (grid_points.left_set().first().unwrap(), grid_points.right_set().first().unwrap());
        let upper_right_grid_point = (grid_points.left_set().last().unwrap(), grid_points.right_set().last().unwrap());
        let lower_left_pixel = lisa_small_border.get_pixel(*lower_left_grid_point.0, *lower_left_grid_point.1);
        let upper_right_pixel = lisa_small_border.get_pixel(*upper_right_grid_point.0, *upper_right_grid_point.1);
        assert!(lower_left_pixel.0[0] < 250);
        assert!(upper_right_pixel.0[0] < 250);

        let lisa_large_border = image::open(image_path("lisa-large_border.jpg"))?.to_luma8();

        let (grid_points, _) = compute_grid_points(&lisa_large_border);
        let lower_left_grid_point = (grid_points.left_set().first().unwrap(), grid_points.right_set().first().unwrap());
        let upper_right_grid_point = (grid_points.left_set().last().unwrap(), grid_points.right_set().last().unwrap());
        let lower_left_pixel = lisa_large_border.get_pixel(*lower_left_grid_point.0, *lower_left_grid_point.1);
        let upper_right_pixel = lisa_large_border.get_pixel(*upper_right_grid_point.0, *upper_right_grid_point.1);
        assert!(lower_left_pixel.0[0] < 250);
        assert!(upper_right_pixel.0[0] < 250);
        Ok(())
    }

    fn matching_indexes(indexes_a: &[i32], indexes_b: &[i32]) -> usize {
        indexes_a.iter().zip(indexes_b.iter()).filter(|(a, b)| a == b).count()
    }

    fn image_properties(asset: &str) -> ImageResult<([i64; COMPRESSED_SIGNATURE_LEN], [i32; NUM_WORDS])> {
        let signature = compute(&image::open(image_path(asset))?);
        let indexes = generate_indexes(&signature);
        Ok((signature, indexes))
    }
}
