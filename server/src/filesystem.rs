use crate::config;
use crate::content::hash::{self, PostHash};
use crate::model::enums::MimeType;
use image::{DynamicImage, ImageResult};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::LazyLock;
use uuid::Uuid;

pub enum Directory {
    Posts,
    GeneratedThumbnails,
    CustomThumbnails,
    CustomAvatars,
    TemporaryUploads,
}

pub enum ThumbnailType {
    Generated,
    Custom,
}

pub fn path(directory: Directory) -> &'static Path {
    Path::new(as_str(directory))
}

pub fn as_str(directory: Directory) -> &'static str {
    match directory {
        Directory::Posts => &POSTS_DIRECTORY,
        Directory::GeneratedThumbnails => &GENERATED_THUMBNAILS_DIRECTORY,
        Directory::CustomThumbnails => &CUSTOM_THUMBNAILS_DIRECTORY,
        Directory::CustomAvatars => &CUSTOM_AVATARS_DIRECTORY,
        Directory::TemporaryUploads => &TEMPORARY_UPLOADS_DIRECTORY,
    }
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

pub fn save_custom_avatar(username: &str, thumbnail: DynamicImage) -> ImageResult<()> {
    assert_eq!(thumbnail.width(), config::get().thumbnails.avatar_width);
    assert_eq!(thumbnail.height(), config::get().thumbnails.avatar_height);

    create_dir(Directory::CustomAvatars)?;
    let avatar_path = hash::custom_avatar_path(username);

    thumbnail.to_rgb8().save(&avatar_path)?;
    let file_size = std::fs::metadata(avatar_path)?.len();

    DATA_SIZE.fetch_add(file_size, Ordering::SeqCst);
    Ok(())
}

pub fn delete_custom_avatar(username: &str) -> std::io::Result<()> {
    let custom_avatar_path = hash::custom_avatar_path(username);
    custom_avatar_path
        .exists()
        .then(|| remove_file(&custom_avatar_path))
        .unwrap_or(Ok(()))
}

pub fn save_post_thumbnail(post: &PostHash, thumbnail: DynamicImage, thumbnail_type: ThumbnailType) -> ImageResult<()> {
    assert_eq!(thumbnail.width(), config::get().thumbnails.post_height);
    assert_eq!(thumbnail.height(), config::get().thumbnails.post_height);

    let thumbnail_path = match thumbnail_type {
        ThumbnailType::Generated => {
            create_dir(Directory::GeneratedThumbnails)?;
            post.generated_thumbnail_path()
        }
        ThumbnailType::Custom => {
            create_dir(Directory::CustomThumbnails)?;
            post.custom_thumbnail_path()
        }
    };

    thumbnail.to_rgb8().save(&thumbnail_path)?;
    let file_size = std::fs::metadata(thumbnail_path)?.len();

    DATA_SIZE.fetch_add(file_size, Ordering::SeqCst);
    Ok(())
}

pub fn delete_post_thumbnail(post: &PostHash, thumbnail_type: ThumbnailType) -> std::io::Result<()> {
    match thumbnail_type {
        ThumbnailType::Generated => remove_file(&post.generated_thumbnail_path()),
        ThumbnailType::Custom => {
            let custom_thumbnail_path = post.custom_thumbnail_path();
            custom_thumbnail_path
                .exists()
                .then(|| remove_file(&custom_thumbnail_path))
                .unwrap_or(Ok(()))
        }
    }
}

pub fn delete_content(post: &PostHash, mime_type: MimeType) -> std::io::Result<()> {
    let content_path = post.content_path(mime_type);
    remove_file(&content_path)
}

pub fn delete_post(post: &PostHash, mime_type: MimeType) -> std::io::Result<()> {
    delete_post_thumbnail(post, ThumbnailType::Generated)?;
    delete_post_thumbnail(post, ThumbnailType::Custom)?;
    delete_content(post, mime_type)
}

/*
    Renames the contents and thumbnails of two posts as if they had swapped ids.
*/
pub fn swap_posts(
    post_a: &PostHash,
    mime_type_a: MimeType,
    post_b: &PostHash,
    mime_type_b: MimeType,
) -> std::io::Result<()> {
    swap_files(&post_a.generated_thumbnail_path(), &post_b.generated_thumbnail_path())?;

    let custom_thumbnail_path_a = post_a.custom_thumbnail_path();
    let custom_thumbnail_path_b = post_b.custom_thumbnail_path();
    match (custom_thumbnail_path_a.exists(), custom_thumbnail_path_b.exists()) {
        (true, true) => swap_files(&custom_thumbnail_path_a, &custom_thumbnail_path_b)?,
        (true, false) => std::fs::rename(custom_thumbnail_path_a, custom_thumbnail_path_b)?,
        (false, true) => std::fs::rename(custom_thumbnail_path_b, custom_thumbnail_path_a)?,
        (false, false) => (),
    }

    let old_image_path_a = post_a.content_path(mime_type_a);
    let old_image_path_b = post_b.content_path(mime_type_b);
    if mime_type_a == mime_type_b {
        swap_files(&old_image_path_a, &old_image_path_b)
    } else {
        std::fs::rename(old_image_path_a, post_b.content_path(mime_type_a))?;
        std::fs::rename(old_image_path_b, post_a.content_path(mime_type_b))
    }
}

/*
    Creates a directory or does nothing if it already exists.
    If no error occured, returns whether a directory was created.
*/
pub fn create_dir(directory: Directory) -> std::io::Result<bool> {
    let path = path(directory);
    match path.exists() {
        true => Ok(false),
        false => std::fs::create_dir(path).map(|_| true),
    }
}

pub fn purge_temporary_uploads() -> std::io::Result<()> {
    let temp_path = path(Directory::TemporaryUploads);
    if !temp_path.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(temp_path)? {
        let path = entry?.path();
        std::fs::remove_file(path)?;
    }
    Ok(())
}

pub fn data_size() -> std::io::Result<u64> {
    Ok(match DATA_SIZE.compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst) {
        Ok(_) => DATA_SIZE.fetch_add(calculate_directory_size(Path::new(config::data_dir()))?, Ordering::SeqCst),
        Err(current_value) => current_value,
    })
}

static DATA_SIZE: AtomicU64 = AtomicU64::new(0);
static POSTS_DIRECTORY: LazyLock<String> = LazyLock::new(|| format!("{}/posts", config::data_dir()).into());
static GENERATED_THUMBNAILS_DIRECTORY: LazyLock<String> =
    LazyLock::new(|| format!("{}/generated-thumbnails", config::data_dir()).into());
static CUSTOM_THUMBNAILS_DIRECTORY: LazyLock<String> =
    LazyLock::new(|| format!("{}/custom-thumbnails", config::data_dir()).into());
static CUSTOM_AVATARS_DIRECTORY: LazyLock<String> =
    LazyLock::new(|| format!("{}/custom-avatars", config::data_dir()).into());
static TEMPORARY_UPLOADS_DIRECTORY: LazyLock<String> =
    LazyLock::new(|| format!("{}/temporary-uploads", config::data_dir()).into());

fn remove_file(path: &Path) -> std::io::Result<()> {
    let file_size = std::fs::metadata(path)?.len();
    std::fs::remove_file(path)?;

    DATA_SIZE.fetch_sub(file_size, Ordering::SeqCst);
    Ok(())
}

fn swap_files(file_a: &Path, file_b: &Path) -> std::io::Result<()> {
    let temp_path =
        Path::new(TEMPORARY_UPLOADS_DIRECTORY.as_str()).join(file_a.file_name().unwrap_or(OsStr::new("post.tmp")));
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
