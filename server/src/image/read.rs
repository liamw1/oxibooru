use image::{ImageReader, Limits};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

pub fn new_image_reader(image_path: &Path) -> std::io::Result<ImageReader<BufReader<File>>> {
    let mut reader = image::ImageReader::open(image_path)?;
    reader.limits(image_reader_limits());
    Ok(reader)
}

fn image_reader_limits() -> Limits {
    const GB: u64 = 1024 * 1024 * 1024;

    let mut limits = Limits::no_limits();
    limits.max_alloc = Some(4 * GB);
    limits
}
