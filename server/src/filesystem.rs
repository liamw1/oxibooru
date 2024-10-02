use crate::auth::hash;
use crate::config;
use crate::model::enums::MimeType;
use image::{DynamicImage, ImageResult};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::LazyLock;
use uuid::Uuid;

pub enum ThumbnailType {
    Generated,
    Custom,
}

pub fn posts_directory() -> &'static Path {
    &POSTS_DIRECTORY
}

pub fn generated_thumbnails_directory() -> &'static Path {
    &GENERATED_THUMBNAILS_DIRECTORY
}

pub fn custom_thumbnails_directory() -> &'static Path {
    &CUSTOM_THUMBNAILS_DIRECTORY
}

pub fn temporary_upload_directory() -> &'static Path {
    &TEMPORARY_UPLOADS_DIRECTORY
}

pub fn temporary_upload_filepath(filename: &str) -> PathBuf {
    format!("{}/temporary-uploads/{}", config::data_dir(), filename).into()
}

pub fn save_uploaded_file(data: Vec<u8>, mime_type: MimeType) -> std::io::Result<String> {
    let upload_token = format!("{}.{}", Uuid::new_v4(), mime_type.extension());
    let upload_path = temporary_upload_filepath(&upload_token);
    std::fs::write(upload_path, &data)?;

    let data_size = size_of::<u8>() * data.len();
    DATA_SIZE.fetch_add(data_size as u64, Ordering::SeqCst);
    Ok(upload_token)
}

pub fn save_thumbnail(post_id: i32, thumbnail: DynamicImage, thumbnail_type: ThumbnailType) -> ImageResult<()> {
    assert_eq!(thumbnail.width(), config::get().thumbnails.post_height);
    assert_eq!(thumbnail.height(), config::get().thumbnails.post_height);

    let thumbnail_path = match thumbnail_type {
        ThumbnailType::Generated => {
            create_dir(generated_thumbnails_directory())?;
            hash::generated_thumbnail_path(post_id)
        }
        ThumbnailType::Custom => {
            create_dir(custom_thumbnails_directory())?;
            hash::custom_thumbnail_path(post_id)
        }
    };

    thumbnail.to_rgb8().save(&thumbnail_path)?;
    let file_size = std::fs::metadata(thumbnail_path)?.len();

    DATA_SIZE.fetch_add(file_size, Ordering::SeqCst);
    Ok(())
}

pub fn delete_thumbnail(post_id: i32, thumbnail_type: ThumbnailType) -> std::io::Result<()> {
    match thumbnail_type {
        ThumbnailType::Generated => remove_file(&hash::generated_thumbnail_path(post_id)),
        ThumbnailType::Custom => {
            let custom_thumbnail_path = hash::custom_thumbnail_path(post_id);
            custom_thumbnail_path
                .exists()
                .then(|| remove_file(&custom_thumbnail_path))
                .unwrap_or(Ok(()))
        }
    }
}

pub fn delete_content(post_id: i32, mime_type: MimeType) -> std::io::Result<()> {
    let content_path = hash::post_content_path(post_id, mime_type);
    remove_file(&content_path)
}

pub fn delete_post(post_id: i32, mime_type: MimeType) -> std::io::Result<()> {
    delete_thumbnail(post_id, ThumbnailType::Generated)?;
    delete_thumbnail(post_id, ThumbnailType::Custom)?;
    delete_content(post_id, mime_type)
}

/*
    Renames the contents and thumbnails of two posts as if they had swapped ids.
*/
pub fn swap_posts(post_id_a: i32, mime_type_a: MimeType, post_id_b: i32, mime_type_b: MimeType) -> std::io::Result<()> {
    swap_files(&hash::generated_thumbnail_path(post_id_a), &hash::generated_thumbnail_path(post_id_b))?;

    let custom_thumbnail_path_a = hash::custom_thumbnail_path(post_id_a);
    let custom_thumbnail_path_b = hash::custom_thumbnail_path(post_id_b);
    match (custom_thumbnail_path_a.exists(), custom_thumbnail_path_b.exists()) {
        (true, true) => swap_files(&custom_thumbnail_path_a, &custom_thumbnail_path_b)?,
        (true, false) => std::fs::rename(custom_thumbnail_path_a, custom_thumbnail_path_b)?,
        (false, true) => std::fs::rename(custom_thumbnail_path_b, custom_thumbnail_path_a)?,
        (false, false) => (),
    }

    let old_image_path_a = hash::post_content_path(post_id_a, mime_type_a);
    let old_image_path_b = hash::post_content_path(post_id_b, mime_type_b);
    if mime_type_a == mime_type_b {
        swap_files(&old_image_path_a, &old_image_path_b)
    } else {
        std::fs::rename(old_image_path_a, hash::post_content_path(post_id_b, mime_type_a))?;
        std::fs::rename(old_image_path_b, hash::post_content_path(post_id_a, mime_type_b))
    }
}

/*
    Creates a directory or does nothing if one already exists.
    If no error occured, returns whether a directory was created.
*/
pub fn create_dir(path: &Path) -> std::io::Result<bool> {
    match path.exists() {
        true => Ok(false),
        false => std::fs::create_dir(path).map(|_| true),
    }
}

pub fn purge_temporary_uploads() -> std::io::Result<()> {
    let temp_path = temporary_upload_directory();
    if !temp_path.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(temporary_upload_directory())? {
        let path = entry?.path();
        std::fs::remove_file(path)?;
    }
    Ok(())
}

pub fn data_size() -> std::io::Result<u64> {
    Ok(match DATA_SIZE.compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst) {
        Ok(_) => {
            DATA_SIZE.fetch_add(calculate_directory_size(posts_directory())?, Ordering::SeqCst);
            DATA_SIZE.fetch_add(calculate_directory_size(generated_thumbnails_directory())?, Ordering::SeqCst);
            DATA_SIZE.fetch_add(calculate_directory_size(temporary_upload_directory())?, Ordering::SeqCst)
        }
        Err(current_value) => current_value,
    })
}

static DATA_SIZE: AtomicU64 = AtomicU64::new(0);
static POSTS_DIRECTORY: LazyLock<PathBuf> = LazyLock::new(|| format!("{}/posts", config::data_dir()).into());
static GENERATED_THUMBNAILS_DIRECTORY: LazyLock<PathBuf> =
    LazyLock::new(|| format!("{}/generated-thumbnails", config::data_dir()).into());
static CUSTOM_THUMBNAILS_DIRECTORY: LazyLock<PathBuf> =
    LazyLock::new(|| format!("{}/custom-thumbnails", config::data_dir()).into());
static TEMPORARY_UPLOADS_DIRECTORY: LazyLock<PathBuf> =
    LazyLock::new(|| format!("{}/temporary-uploads", config::data_dir()).into());

fn remove_file(path: &Path) -> std::io::Result<()> {
    let file_size = std::fs::metadata(path)?.len();
    std::fs::remove_file(path)?;

    DATA_SIZE.fetch_sub(file_size, Ordering::SeqCst);
    Ok(())
}

fn swap_files(file_a: &Path, file_b: &Path) -> std::io::Result<()> {
    let temp_path = TEMPORARY_UPLOADS_DIRECTORY.join(file_a.file_name().unwrap_or(OsStr::new("post.tmp")));
    std::fs::rename(file_a, &temp_path)?;
    std::fs::rename(file_b, file_a)?;
    std::fs::rename(temp_path, file_b)
}

fn calculate_directory_size(path: &Path) -> std::io::Result<u64> {
    if !path.exists() {
        return Ok(0);
    }

    let mut total_size = 0;
    if path.is_dir() {
        for entry in std::fs::read_dir(path)? {
            let path = entry?.path();
            total_size += calculate_directory_size(&path)?;
        }
    } else {
        total_size += std::fs::metadata(path)?.len();
    }
    Ok(total_size)
}
