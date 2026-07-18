use crate::api::error::{ApiError, ApiResult};
use crate::config::Config;
use crate::content::hash::PostHash;
use crate::content::thumbnail::ThumbnailCategory;
use crate::content::upload::UploadToken;
use crate::model::enums::MimeType;
use axum::body::Bytes;
use futures::StreamExt;
use image::error::ImageError;
use image::{DynamicImage, ImageResult};
use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::io::ErrorKind;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use strum::{Display, IntoStaticStr};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::time::MissedTickBehavior;
use tracing::warn;

/// Represents important data directories.
#[derive(Clone, Copy, Display, IntoStaticStr)]
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

/// Saves streamed file contents to the temporary uploads folder as a `mime_type` file.
/// Returns the name of the file written.
///
/// Does not perform cleanup on error. It instead relies on the cleanup task spawned from
/// `spawn_temporary_uploads_cleanup_task` to clean out failed uploads.
pub async fn save_uploaded_file<S, E>(config: &Config, mut stream: S, mime_type: MimeType) -> ApiResult<UploadToken>
where
    S: StreamExt<Item = Result<Bytes, E>> + Unpin,
    ApiError: From<E>,
{
    std::fs::create_dir_all(config.path(Directory::TemporaryUploads))?;

    let upload_token = UploadToken::new(mime_type);
    let upload_path = upload_token.path(config);

    let mut file = File::create(upload_path).await?;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        file.write_all(&chunk).await?;
    }
    file.flush().await?;

    Ok(upload_token)
}

/// Saves custom avatar `thumbnail` for user with name `username` to disk.
/// Returns size of the thumbnail in bytes.
pub fn save_custom_avatar(config: &Config, lowercase_username: &str, thumbnail: DynamicImage) -> ImageResult<i64> {
    std::fs::create_dir_all(config.path(Directory::Avatars))?;

    let avatar_path = config.custom_avatar_path(lowercase_username);
    thumbnail.into_rgb8().save(&avatar_path)?;
    file_size(&avatar_path).map_err(ImageError::from)
}

/// Deletes custom avatar for user with name `username` from disk, if it exists.
pub fn delete_custom_avatar(config: &Config, lowercase_username: &str) -> std::io::Result<()> {
    let custom_avatar_path = config.custom_avatar_path(lowercase_username);
    remove_if_exists(&custom_avatar_path)
}

/// Saves `post` `thumbnail` to disk. Can be custom or automatically generated.
/// Returns size of the thumbnail in bytes.
pub fn save_post_thumbnail(
    post: &PostHash,
    thumbnail: DynamicImage,
    thumbnail_type: ThumbnailCategory,
) -> ImageResult<i64> {
    let thumbnail_path = match thumbnail_type {
        ThumbnailCategory::Generated => post.generated_thumbnail_path(),
        ThumbnailCategory::Custom => post.custom_thumbnail_path(),
    };
    std::fs::create_dir_all(thumbnail_path.parent().unwrap_or(Path::new("")))?;

    thumbnail.into_rgb8().save(&thumbnail_path)?;
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
    remove_if_exists(&content_path)
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
    std::fs::create_dir_all(to.parent().unwrap_or(Path::new("")))?;
    if let Err(err) = std::fs::rename(from, to) {
        if err.kind() == ErrorKind::CrossesDevices {
            std::fs::copy(from, to)?;
            std::fs::remove_file(from)?;
        } else {
            return Err(err);
        }
    }

    // Set appropriate permissions since we usually use this function to move
    // content to a permanent location
    if let Err(err) = set_permissions(to) {
        warn!("Failed to set permissions for {} for reason: {err}", to.display());
    }
    Ok(())
}

/// Spawns an asynchronous task that periodically checks the temporary
/// upload directory for stale file uploads and deletes them.
pub fn spawn_temporary_uploads_cleanup_task(config: Arc<Config>) {
    const SWEEP_INTERVAL: Duration = Duration::from_hours(1);

    tokio::spawn(async move {
        let mut uploads = HashMap::new();
        let mut interval = tokio::time::interval(SWEEP_INTERVAL);
        interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            interval.tick().await;
            remove_stale_uploads(&config, &mut uploads);
        }
    });
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

/// Removes any stale files in the temporary uploads directory that are contained within `uploads`.
fn remove_stale_uploads(config: &Config, uploads: &mut HashMap<PathBuf, u64>) {
    let temporary_uploads_path = config.path(Directory::TemporaryUploads);
    let directory_iter = match std::fs::read_dir(temporary_uploads_path) {
        Ok(iter) => iter,
        Err(err) => {
            if err.kind() == ErrorKind::NotFound {
                // Directory must have been deleted after startup. Clear uploads map
                uploads.clear();
            } else {
                warn!("Failed to cleanup temporary uploads directory: {err}");
            }
            return;
        }
    };

    let mut seen_files = HashSet::new();
    for file in directory_iter {
        let path = match file {
            Ok(entry) => entry.path(),
            Err(err) => {
                warn!("Failed to read directory entry: {err}");
                continue;
            }
        };
        let filesize = match path.metadata() {
            Ok(metadata) => metadata.len(),
            Err(err) => {
                if err.kind() != ErrorKind::NotFound {
                    warn!("Failed to read metadata for {}: {err}", path.display());
                    seen_files.insert(path);
                }
                continue;
            }
        };

        match uploads.entry(path.clone()) {
            Entry::Occupied(mut entry) => {
                // If filesize has grown, assume file is still downloading and don't delete
                if filesize > *entry.get() {
                    *entry.get_mut() = filesize;
                    seen_files.insert(path);
                } else if let Err(err) = remove_if_exists(&path) {
                    warn!("Failed to remove {}: {err}", path.display());
                    seen_files.insert(path);
                } else {
                    entry.remove();
                }
            }
            Entry::Vacant(entry) => {
                entry.insert(filesize);
                seen_files.insert(path);
            }
        }
    }

    // Drop entries for files that no longer exist
    uploads.retain(|path, _| seen_files.contains(path));
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

/// Makes `path` readable to world. Used to avoid permissions issues on some systems.
fn set_permissions(path: &Path) -> std::io::Result<()> {
    const STANDARD_PERMISSIONS: u32 = 0o644;

    let mut permissions = std::fs::metadata(path)?.permissions();
    permissions.set_mode(STANDARD_PERMISSIONS);
    std::fs::set_permissions(path, permissions)
}
