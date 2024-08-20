use crate::auth::content;
use crate::config;
use crate::model::enums::MimeType;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::LazyLock;
use uuid::Uuid;

pub fn posts_directory() -> &'static Path {
    &POSTS_DIRECTORY
}

pub fn generated_thumbnails_directory() -> &'static Path {
    &THUMBNAILS_DIRECTORY
}

pub fn temporary_upload_directory() -> &'static Path {
    &TEMPORARY_UPLOADS_DIRECTORY
}

pub fn temporary_upload_filepath(filename: &str) -> PathBuf {
    format!("{}/temporary-uploads/{}", config::get().data_dir, filename).into()
}

pub fn upload(data: &[u8], content_type: MimeType) -> std::io::Result<String> {
    let upload_token = format!("{}.{}", Uuid::new_v4(), content_type.extension());
    let upload_path = temporary_upload_filepath(&upload_token);
    std::fs::write(upload_path, data)?;

    DATA_SIZE.fetch_add(data.len() as u64, Ordering::SeqCst);
    Ok(upload_token)
}

pub fn remove_post(post_id: i32, mime_type: MimeType) -> std::io::Result<()> {
    let thumbnail_path = content::post_thumbnail_path(post_id);
    let image_path = content::post_content_path(post_id, mime_type);
    remove_file(&thumbnail_path)?;
    remove_file(&image_path)?;
    Ok(())
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
    if DATA_SIZE
        .compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
    {
        DATA_SIZE.fetch_add(calculate_directory_size(posts_directory())?, Ordering::SeqCst);
        DATA_SIZE.fetch_add(calculate_directory_size(generated_thumbnails_directory())?, Ordering::SeqCst);
        DATA_SIZE.fetch_add(calculate_directory_size(temporary_upload_directory())?, Ordering::SeqCst);
    }
    Ok(DATA_SIZE.load(Ordering::SeqCst))
}

static DATA_SIZE: AtomicU64 = AtomicU64::new(0);
static POSTS_DIRECTORY: LazyLock<PathBuf> = LazyLock::new(|| format!("{}/posts", config::get().data_dir).into());
static THUMBNAILS_DIRECTORY: LazyLock<PathBuf> =
    LazyLock::new(|| format!("{}/generated-thumbnails", config::get().data_dir).into());
static TEMPORARY_UPLOADS_DIRECTORY: LazyLock<PathBuf> =
    LazyLock::new(|| format!("{}/temporary-uploads", config::get().data_dir).into());

fn remove_file(path: &Path) -> std::io::Result<()> {
    let file_size = std::fs::metadata(path)?.len();
    std::fs::remove_file(path)?;

    DATA_SIZE.fetch_sub(file_size, Ordering::SeqCst);
    Ok(())
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
