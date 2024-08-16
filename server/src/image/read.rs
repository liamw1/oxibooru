use image::ImageReader;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

pub fn new_image_reader(image_path: &Path) -> std::io::Result<ImageReader<BufReader<File>>> {
    let mut reader = image::ImageReader::open(image_path)?;
    reader.no_limits(); // TODO: Set reasonable limits
    Ok(reader)
}
