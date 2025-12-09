use crate::config::Config;
use crate::content::hash::PostHash;
use crate::content::thumbnail::ThumbnailCategory;
use crate::model::enums::MimeType;
use image::error::ImageError;
use image::{DynamicImage, ImageResult};
use std::ffi::OsStr;
use std::io::ErrorKind;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use strum::IntoStaticStr;
use tracing::warn;
use uuid::Uuid;

/// Represents important data directories.
#[derive(Clone, Copy, IntoStaticStr)]
#[strum(serialize_all = "kebab-case")]
pub enum Directory {
    Avatars,
    Posts,
    GeneratedThumbnails,
    CustomThumbnails,
    TemporaryUploads,
}

/// Returns the size of the file at `path` in bytes as an i64
pub fn file_size(path: &Path) -> std::io::Result<i64> {
    path.metadata()
        .map(|metadata| i64::try_from(metadata.len()).expect("File size must be less than i64::MAX"))
}

/// Saves raw bytes to temporary upload folder as a `mime_type`-file to disk.
/// Returns name of the file written.
pub fn save_uploaded_file(config: &Config, data: &[u8], mime_type: MimeType) -> std::io::Result<String> {
    let upload_token = format!("{}.{}", Uuid::new_v4(), mime_type.extension());
    let upload_path = config.path(Directory::TemporaryUploads).join(&upload_token);
    create_parent_directories(&upload_path)?;

    std::fs::write(upload_path, data)?;
    Ok(upload_token)
}

/// Saves custom avatar `thumbnail` for user with name `username` to disk.
/// Returns size of the thumbnail in bytes.
pub fn save_custom_avatar(config: &Config, username: &str, thumbnail: &DynamicImage) -> ImageResult<i64> {
    let avatar_path = config.custom_avatar_path(username);
    create_parent_directories(&avatar_path)?;

    thumbnail.to_rgb8().save(&avatar_path)?;
    file_size(&avatar_path).map_err(ImageError::from)
}

/// Deletes custom avatar for user with name `username` from disk, if it exists.
pub fn delete_custom_avatar(config: &Config, username: &str) -> std::io::Result<()> {
    let custom_avatar_path = config.custom_avatar_path(username);
    remove_if_exists(&custom_avatar_path)
}

/// Saves `post` `thumbnail` to disk. Can be custom or automatically generated.
/// Returns size of the thumbnail in bytes.
pub fn save_post_thumbnail(
    post: &PostHash,
    thumbnail: &DynamicImage,
    thumbnail_type: ThumbnailCategory,
) -> ImageResult<i64> {
    let thumbnail_path = match thumbnail_type {
        ThumbnailCategory::Generated => post.generated_thumbnail_path(),
        ThumbnailCategory::Custom => post.custom_thumbnail_path(),
    };
    create_parent_directories(&thumbnail_path)?;

    thumbnail.to_rgb8().save(&thumbnail_path)?;
    file_size(&thumbnail_path).map_err(ImageError::from)
}

/// Deletes thumbnail of `post` from disk, if it exists.
pub fn delete_post_thumbnail(post: &PostHash, thumbnail_type: ThumbnailCategory) -> std::io::Result<()> {
    let thumbnail_path = match thumbnail_type {
        ThumbnailCategory::Generated => post.generated_thumbnail_path(),
        ThumbnailCategory::Custom => post.custom_thumbnail_path(),
    };
    remove_if_exists(&thumbnail_path)
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
    config: &Config,
    post_a: &PostHash,
    mime_type_a: MimeType,
    post_b: &PostHash,
    mime_type_b: MimeType,
) -> std::io::Result<()> {
    // No special cases needed here because generated thumbnails always exists and their type is always .jpg
    swap_files(config, &post_a.generated_thumbnail_path(), &post_b.generated_thumbnail_path())?;

    // Handle the four distinct cases of custom thumbnails existing/not existing
    let custom_thumbnail_path_a = post_a.custom_thumbnail_path();
    let custom_thumbnail_path_b = post_b.custom_thumbnail_path();
    match (custom_thumbnail_path_a.try_exists()?, custom_thumbnail_path_b.try_exists()?) {
        (true, true) => swap_files(config, &custom_thumbnail_path_a, &custom_thumbnail_path_b)?,
        (true, false) => move_file(&custom_thumbnail_path_a, &custom_thumbnail_path_b)?,
        (false, true) => move_file(&custom_thumbnail_path_b, &custom_thumbnail_path_a)?,
        (false, false) => (),
    }

    // Contents can have same MIME type or different MIME types
    let old_image_path_a = post_a.content_path(mime_type_a);
    let old_image_path_b = post_b.content_path(mime_type_b);
    if mime_type_a == mime_type_b {
        swap_files(config, &old_image_path_a, &old_image_path_b)
    } else {
        move_file(&old_image_path_a, &post_b.content_path(mime_type_a))?;
        move_file(&old_image_path_b, &post_a.content_path(mime_type_b))
    }
}

/// Moves file from `from` to `to`.
/// Tries simply renaming first and falls back to copy/remove if `from` and `to`
/// are on different file systems.
pub fn move_file(from: &Path, to: &Path) -> std::io::Result<()> {
    create_parent_directories(to)?;
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
pub fn purge_temporary_uploads(config: &Config) -> std::io::Result<()> {
    let temporary_uploads_path = config.path(Directory::TemporaryUploads);
    if temporary_uploads_path.try_exists()? {
        for entry in std::fs::read_dir(config.path(Directory::TemporaryUploads))? {
            let path = entry?.path();
            std::fs::remove_file(path)?;
        }
    }
    Ok(())
}

/// Removes `file` if it exists.
fn remove_if_exists(file: &Path) -> std::io::Result<()> {
    if let Err(err) = std::fs::remove_file(file)
        && err.kind() != std::io::ErrorKind::NotFound
    {
        Err(err)
    } else {
        Ok(())
    }
}

/// Swaps the names of two files.
fn swap_files(config: &Config, file_a: &Path, file_b: &Path) -> std::io::Result<()> {
    let temp_path = config
        .path(Directory::TemporaryUploads)
        .join(file_a.file_name().unwrap_or(OsStr::new("post.tmp")));
    move_file(file_a, &temp_path)?;
    move_file(file_b, file_a)?;
    move_file(&temp_path, file_b)
}

fn set_permissions(path: &Path) -> std::io::Result<()> {
    let mut permissions = std::fs::metadata(path)?.permissions();
    permissions.set_mode(0o644);
    std::fs::set_permissions(path, permissions)
}

fn create_parent_directories(path: &Path) -> std::io::Result<()> {
    if let Err(err) = std::fs::create_dir_all(path.parent().unwrap_or(Path::new("")))
        && err.kind() != std::io::ErrorKind::AlreadyExists
    {
        Err(err)
    } else {
        Ok(())
    }
}
