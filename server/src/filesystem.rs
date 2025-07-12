use crate::config;
use crate::content::hash::{self, PostHash};
use crate::content::thumbnail::ThumbnailCategory;
use crate::model::enums::MimeType;
use image::{DynamicImage, ImageResult};
use std::ffi::OsStr;
use std::io::ErrorKind;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use tracing::warn;
use uuid::Uuid;

/// Represents important data directories.
pub enum Directory {
    Avatars,
    Posts,
    GeneratedThumbnails,
    CustomThumbnails,
    TemporaryUploads,
}

/// Returns absolute path to the specified `directory`.
pub fn path(directory: Directory) -> &'static Path {
    Path::new(as_str(directory))
}

/// Returns absolute path to the specified `directory` as a str.
pub fn as_str(directory: Directory) -> &'static str {
    match directory {
        Directory::Avatars => &AVATARS_DIRECTORY,
        Directory::Posts => &POSTS_DIRECTORY,
        Directory::GeneratedThumbnails => &GENERATED_THUMBNAILS_DIRECTORY,
        Directory::CustomThumbnails => &CUSTOM_THUMBNAILS_DIRECTORY,
        Directory::TemporaryUploads => &TEMPORARY_UPLOADS_DIRECTORY,
    }
}

/// Creates a path to a temporary upload with the given `filename`.
pub fn temporary_upload_filepath(filename: &str) -> PathBuf {
    format!("{}/temporary-uploads/{}", config::data_dir(), filename).into()
}

/// Saves raw bytes to temporary upload folder as a `mime_type`-file to disk.
/// Returns name of the file written.
pub fn save_uploaded_file(data: &[u8], mime_type: MimeType) -> std::io::Result<String> {
    // Create temp directory if necessary
    create_dir(Directory::TemporaryUploads)?;

    let upload_token = format!("{}.{}", Uuid::new_v4(), mime_type.extension());
    let upload_path = temporary_upload_filepath(&upload_token);
    std::fs::write(upload_path, data)?;
    Ok(upload_token)
}

/// Saves custom avatar `thumbnail` for user with name `username` to disk.
/// Returns size of the thumbnail in bytes.
pub fn save_custom_avatar(username: &str, thumbnail: DynamicImage) -> ImageResult<u64> {
    create_dir(Directory::Avatars)?;
    let avatar_path = hash::custom_avatar_path(username);

    thumbnail.to_rgb8().save(&avatar_path)?;
    let metadata = std::fs::metadata(avatar_path)?;
    Ok(metadata.len())
}

/// Deletes custom avatar for user with name `username` from disk.
pub fn delete_custom_avatar(username: &str) -> std::io::Result<()> {
    let custom_avatar_path = hash::custom_avatar_path(username);
    match custom_avatar_path.try_exists()? {
        true => std::fs::remove_file(&custom_avatar_path),
        false => Ok(()),
    }
}

/// Saves `post` `thumbnail` to disk. Can be custom or automatically generated.
/// Returns size of the thumbnail in bytes.
pub fn save_post_thumbnail(
    post: &PostHash,
    thumbnail: DynamicImage,
    thumbnail_type: ThumbnailCategory,
) -> ImageResult<u64> {
    let thumbnail_path = match thumbnail_type {
        ThumbnailCategory::Generated => {
            create_dir(Directory::GeneratedThumbnails)?;
            post.generated_thumbnail_path()
        }
        ThumbnailCategory::Custom => {
            create_dir(Directory::CustomThumbnails)?;
            post.custom_thumbnail_path()
        }
    };

    thumbnail.to_rgb8().save(&thumbnail_path)?;
    let metadata = std::fs::metadata(thumbnail_path)?;
    Ok(metadata.len())
}

/// Deletes thumbnail of `post` from disk.
/// Returns error if thumbnail does not exist and `thumbnail_type` is [ThumbnailType::Generated].
pub fn delete_post_thumbnail(post: &PostHash, thumbnail_type: ThumbnailCategory) -> std::io::Result<()> {
    match thumbnail_type {
        ThumbnailCategory::Generated => std::fs::remove_file(post.generated_thumbnail_path()),
        ThumbnailCategory::Custom => {
            let custom_thumbnail_path = post.custom_thumbnail_path();
            match post.custom_thumbnail_path().try_exists()? {
                true => std::fs::remove_file(&custom_thumbnail_path),
                false => Ok(()),
            }
        }
    }
}

/// Deletes `post` content from disk.
pub fn delete_content(post: &PostHash, mime_type: MimeType) -> std::io::Result<()> {
    let content_path = post.content_path(mime_type);
    std::fs::remove_file(content_path)
}

/// Deletes `post` thumbnails and content from disk.
pub fn delete_post(post: &PostHash, mime_type: MimeType) -> std::io::Result<()> {
    delete_post_thumbnail(post, ThumbnailCategory::Generated)?;
    delete_post_thumbnail(post, ThumbnailCategory::Custom)?;
    delete_content(post, mime_type)
}

/// Renames the contents and thumbnails of two posts as if they had swapped ids.
pub fn swap_posts(
    post_a: &PostHash,
    mime_type_a: MimeType,
    post_b: &PostHash,
    mime_type_b: MimeType,
) -> std::io::Result<()> {
    // No special cases needed here because generated thumbnails always exists and their type is always .jpg
    swap_files(&post_a.generated_thumbnail_path(), &post_b.generated_thumbnail_path())?;

    // Handle the four distinct cases of custom thumbnails existing/not existing
    let custom_thumbnail_path_a = post_a.custom_thumbnail_path();
    let custom_thumbnail_path_b = post_b.custom_thumbnail_path();
    match (custom_thumbnail_path_a.try_exists()?, custom_thumbnail_path_b.try_exists()?) {
        (true, true) => swap_files(&custom_thumbnail_path_a, &custom_thumbnail_path_b)?,
        (true, false) => move_file(&custom_thumbnail_path_a, &custom_thumbnail_path_b)?,
        (false, true) => move_file(&custom_thumbnail_path_b, &custom_thumbnail_path_a)?,
        (false, false) => (),
    }

    // Contents can have same MIME type or different MIME types
    let old_image_path_a = post_a.content_path(mime_type_a);
    let old_image_path_b = post_b.content_path(mime_type_b);
    if mime_type_a == mime_type_b {
        swap_files(&old_image_path_a, &old_image_path_b)
    } else {
        move_file(&old_image_path_a, &post_b.content_path(mime_type_a))?;
        move_file(&old_image_path_b, &post_a.content_path(mime_type_b))
    }
}

/// Creates `directory` or does nothing if it already exists.
/// Returns whether `directory` was created, or an error if one occured.
pub fn create_dir(directory: Directory) -> std::io::Result<bool> {
    let path = path(directory);
    match path.try_exists()? {
        true => Ok(false),
        false => std::fs::create_dir(path).map(|_| true),
    }
}

/// Moves file from `from` to `to`.
/// Tries simply renaming first and falls back to copy/remove if `from` and `to`
/// are on different file systems.
pub fn move_file(from: &Path, to: &Path) -> std::io::Result<()> {
    if let Err(ErrorKind::CrossesDevices) = std::fs::rename(from, to).as_ref().map_err(std::io::Error::kind) {
        std::fs::copy(from, to)?;
        std::fs::remove_file(from)?;
    }

    // Set appropriate permissions since we usually use this function to move
    // content to a permanent location
    if let Err(err) = set_permissions(to) {
        warn!("Failed to set permissions for {to:?} for reason: {err}");
    }
    Ok(())
}

/// Deletes everything in the temporary uploads directory.
pub fn purge_temporary_uploads() -> std::io::Result<()> {
    let temp_path = path(Directory::TemporaryUploads);
    if !temp_path.try_exists()? {
        return Ok(());
    }
    for entry in std::fs::read_dir(temp_path)? {
        let path = entry?.path();
        std::fs::remove_file(path)?;
    }
    Ok(())
}

static AVATARS_DIRECTORY: LazyLock<String> = LazyLock::new(|| format!("{}/avatars", config::data_dir()));
static POSTS_DIRECTORY: LazyLock<String> = LazyLock::new(|| format!("{}/posts", config::data_dir()));
static GENERATED_THUMBNAILS_DIRECTORY: LazyLock<String> =
    LazyLock::new(|| format!("{}/generated-thumbnails", config::data_dir()));
static CUSTOM_THUMBNAILS_DIRECTORY: LazyLock<String> =
    LazyLock::new(|| format!("{}/custom-thumbnails", config::data_dir()));
static TEMPORARY_UPLOADS_DIRECTORY: LazyLock<String> =
    LazyLock::new(|| format!("{}/temporary-uploads", config::data_dir()));

/// Swaps the names of two files.
fn swap_files(file_a: &Path, file_b: &Path) -> std::io::Result<()> {
    let temp_path =
        Path::new(TEMPORARY_UPLOADS_DIRECTORY.as_str()).join(file_a.file_name().unwrap_or(OsStr::new("post.tmp")));
    move_file(file_a, &temp_path)?;
    move_file(file_b, file_a)?;
    move_file(&temp_path, file_b)
}

fn set_permissions(path: &Path) -> std::io::Result<()> {
    let mut permissions = std::fs::metadata(path)?.permissions();
    permissions.set_mode(0o644);
    std::fs::set_permissions(path, permissions)
}
