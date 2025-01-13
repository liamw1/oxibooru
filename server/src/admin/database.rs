use crate::admin::{ProgressReporter, PRINT_INTERVAL};
use crate::api::ApiResult;
use crate::content::hash::PostHash;
use crate::filesystem::Directory;
use crate::model::enums::MimeType;
use crate::schema::{
    comment, comment_score, comment_statistics, database_statistics, pool, pool_category, pool_category_statistics,
    pool_post, pool_statistics, post, post_favorite, post_feature, post_note, post_relation, post_score,
    post_statistics, post_tag, tag, tag_category, tag_category_statistics, tag_implication, tag_statistics,
    tag_suggestion, user,
};
use crate::time::{DateTime, Timer};
use crate::{admin, filesystem};
use diesel::dsl::{count, max, sum};
use diesel::prelude::*;

/// Renames post files and thumbnails.
/// Useful when the content hash changes.
pub fn reset_filenames() -> std::io::Result<()> {
    let _timer = Timer::new("reset_filenames");

    if filesystem::path(Directory::GeneratedThumbnails).try_exists()? {
        let mut progress = ProgressReporter::new("Generated thumbnails renamed", PRINT_INTERVAL);
        for entry in std::fs::read_dir(filesystem::path(Directory::GeneratedThumbnails))? {
            let path = entry?.path();
            let post_id = match admin::get_post_id(&path) {
                Some(id) => id,
                None => {
                    eprintln!("ERROR: Could not find post_id of {path:?}");
                    continue;
                }
            };

            let new_path = PostHash::new(post_id).generated_thumbnail_path();
            if path != new_path {
                std::fs::rename(path, new_path)?;
            }
            progress.increment();
        }
    }

    if filesystem::path(Directory::CustomThumbnails).try_exists()? {
        let mut progress = ProgressReporter::new("Custom thumbnails renamed", PRINT_INTERVAL);
        for entry in std::fs::read_dir(filesystem::path(Directory::CustomThumbnails))? {
            let path = entry?.path();
            let post_id = match admin::get_post_id(&path) {
                Some(id) => id,
                None => {
                    eprintln!("ERROR: Could not find post_id of {path:?}");
                    continue;
                }
            };

            let new_path = PostHash::new(post_id).custom_thumbnail_path();
            if path != new_path {
                std::fs::rename(path, new_path)?;
            }
            progress.increment();
        }
    }

    if filesystem::path(Directory::Posts).try_exists()? {
        let mut progress = ProgressReporter::new("Posts renamed", PRINT_INTERVAL);
        for entry in std::fs::read_dir(filesystem::path(Directory::Posts))? {
            let path = entry?.path();
            let (post_id, mime_type) = match (admin::get_post_id(&path), MimeType::from_path(&path)) {
                (Some(id), Some(mime_type)) => (id, mime_type),
                _ => {
                    eprintln!("ERROR: Could not find post_id or mime_type of {path:?}");
                    continue;
                }
            };

            let new_path = PostHash::new(post_id).content_path(mime_type);
            if path != new_path {
                std::fs::rename(path, new_path)?;
            }
            progress.increment();
        }
    }

    Ok(())
}

/// Recalculates cached file sizes, row counts, and table statistics.
/// Useful for when the statistics become inconsistent with database
/// or when migrating from an older version without statistics.
pub fn reset_statistics(conn: &mut PgConnection) -> ApiResult<()> {
    let _timer = Timer::new("reset_statistics");

    // Disk usage will automatically be incremented via triggers as we calculate
    // content, thumbnail, and avatar sizes
    diesel::update(database_statistics::table)
        .set(database_statistics::disk_usage.eq(0))
        .execute(conn)?;

    if filesystem::path(Directory::Avatars).try_exists()? {
        let mut progress = ProgressReporter::new("Avatar sizes cached", PRINT_INTERVAL);
        for entry in std::fs::read_dir(filesystem::path(Directory::Avatars))? {
            let path = entry?.path();
            let username = match path.file_name() {
                Some(name) => name.to_string_lossy(),
                None => {
                    eprintln!("ERROR: Unable to convert file name of {path:?} to string");
                    continue;
                }
            };

            let file_size = path.metadata()?.len();
            diesel::update(user::table)
                .set(user::custom_avatar_size.eq(file_size as i64))
                .filter(user::name.eq(username))
                .execute(conn)?;
            progress.increment();
        }
    }

    if filesystem::path(Directory::GeneratedThumbnails).try_exists()? {
        let mut progress = ProgressReporter::new("Generated thumbnail sizes cached", PRINT_INTERVAL);
        for entry in std::fs::read_dir(filesystem::path(Directory::GeneratedThumbnails))? {
            let path = entry?.path();
            let post_id = match admin::get_post_id(&path) {
                Some(id) => id,
                None => {
                    eprintln!("ERROR: Could not find post_id of {path:?}");
                    continue;
                }
            };

            let file_size = path.metadata()?.len();
            diesel::update(post::table)
                .set(post::generated_thumbnail_size.eq(file_size as i64))
                .filter(post::id.eq(post_id))
                .execute(conn)?;
            progress.increment();
        }
    }

    if filesystem::path(Directory::CustomThumbnails).try_exists()? {
        let mut progress = ProgressReporter::new("Custom thumbnails sizes cached", PRINT_INTERVAL);
        for entry in std::fs::read_dir(filesystem::path(Directory::CustomThumbnails))? {
            let path = entry?.path();
            let post_id = match admin::get_post_id(&path) {
                Some(id) => id,
                None => {
                    eprintln!("ERROR: Could not find post_id of {path:?}");
                    continue;
                }
            };

            let file_size = path.metadata()?.len();
            diesel::update(post::table)
                .set(post::custom_thumbnail_size.eq(file_size as i64))
                .filter(post::id.eq(post_id))
                .execute(conn)?;
            progress.increment();
        }
    }

    if filesystem::path(Directory::Posts).try_exists()? {
        let mut progress = ProgressReporter::new("Posts content sizes cached", PRINT_INTERVAL);
        for entry in std::fs::read_dir(filesystem::path(Directory::Posts))? {
            let path = entry?.path();
            let post_id = match admin::get_post_id(&path) {
                Some(id) => id,
                None => {
                    eprintln!("ERROR: Could not find post_id of {path:?}");
                    continue;
                }
            };

            let file_size = path.metadata()?.len();
            diesel::update(post::table)
                .set(post::file_size.eq(file_size as i64))
                .filter(post::id.eq(post_id))
                .execute(conn)?;
            progress.increment();
        }
    }

    let comment_count: i64 = comment::table.count().first(conn)?;
    let pool_count: i64 = pool::table.count().first(conn)?;
    let post_count: i64 = post::table.count().first(conn)?;
    let tag_count: i64 = tag::table.count().first(conn)?;
    let user_count: i64 = user::table.count().first(conn)?;
    diesel::update(database_statistics::table)
        .set((
            database_statistics::comment_count.eq(comment_count as i32),
            database_statistics::pool_count.eq(pool_count as i32),
            database_statistics::post_count.eq(post_count as i32),
            database_statistics::tag_count.eq(tag_count as i32),
            database_statistics::user_count.eq(user_count as i32),
        ))
        .execute(conn)?;

    let comment_stats: Vec<(i32, Option<i64>)> = comment::table
        .left_join(comment_score::table)
        .group_by(comment::id)
        .select((comment::id, count(comment_score::user_id).nullable()))
        .load(conn)?;
    for (comment_id, score) in comment_stats {
        diesel::update(comment_statistics::table)
            .set(comment_statistics::score.eq(score.unwrap_or(0) as i32))
            .filter(comment_statistics::comment_id.eq(comment_id))
            .execute(conn)?;
    }

    let pool_category_stats: Vec<(i32, Option<i64>)> = pool_category::table
        .left_join(pool::table)
        .group_by(pool_category::id)
        .select((pool_category::id, count(pool::id).nullable()))
        .load(conn)?;
    for (category_id, usage_count) in pool_category_stats {
        diesel::update(pool_category_statistics::table)
            .set(pool_category_statistics::usage_count.eq(usage_count.unwrap_or(0) as i32))
            .filter(pool_category_statistics::category_id.eq(category_id))
            .execute(conn)?;
    }

    let pool_stats: Vec<(i32, Option<i64>)> = pool::table
        .left_join(pool_post::table)
        .group_by(pool::id)
        .select((pool::id, count(pool_post::post_id).nullable()))
        .load(conn)?;
    for (pool_id, post_count) in pool_stats {
        diesel::update(pool_statistics::table)
            .set(pool_statistics::post_count.eq(post_count.unwrap_or(0) as i32))
            .filter(pool_statistics::pool_id.eq(pool_id))
            .execute(conn)?;
    }

    let tag_category_stats: Vec<(i32, Option<i64>)> = tag_category::table
        .left_join(tag::table)
        .group_by(tag_category::id)
        .select((tag_category::id, count(tag::id).nullable()))
        .load(conn)?;
    for (category_id, usage_count) in tag_category_stats {
        diesel::update(tag_category_statistics::table)
            .set(tag_category_statistics::usage_count.eq(usage_count.unwrap_or(0) as i32))
            .filter(tag_category_statistics::category_id.eq(category_id))
            .execute(conn)?;
    }

    let tag_ids: Vec<i32> = tag::table.select(tag::id).load(conn)?;
    for tag_id in tag_ids {
        let usage_count: i64 = post_tag::table
            .count()
            .filter(post_tag::tag_id.eq(tag_id))
            .first(conn)?;
        let implication_count: i64 = tag_implication::table
            .count()
            .filter(tag_implication::child_id.eq(tag_id))
            .first(conn)?;
        let suggestion_count: i64 = tag_suggestion::table
            .count()
            .filter(tag_suggestion::child_id.eq(tag_id))
            .first(conn)?;
        diesel::update(tag_statistics::table)
            .set((
                tag_statistics::usage_count.eq(usage_count as i32),
                tag_statistics::implication_count.eq(implication_count as i32),
                tag_statistics::suggestion_count.eq(suggestion_count as i32),
            ))
            .filter(tag_statistics::tag_id.eq(tag_id))
            .execute(conn)?;
    }

    let post_ids: Vec<i32> = post::table.select(post::id).load(conn)?;
    for post_id in post_ids {
        let tag_count: i64 = post_tag::table
            .count()
            .filter(post_tag::post_id.eq(post_id))
            .first(conn)?;
        let pool_count: i64 = pool_post::table
            .count()
            .filter(pool_post::post_id.eq(post_id))
            .first(conn)?;
        let note_count: i64 = post_note::table
            .count()
            .filter(post_note::post_id.eq(post_id))
            .first(conn)?;
        let comment_count: i64 = comment::table
            .count()
            .filter(comment::post_id.eq(post_id))
            .first(conn)?;
        let relation_count: i64 = post_relation::table
            .count()
            .filter(post_relation::child_id.eq(post_id))
            .first(conn)?;
        let score: Option<i64> = post_score::table
            .select(sum(post_score::score))
            .filter(post_score::post_id.eq(post_id))
            .first(conn)?;
        let favorite_count: i64 = post_favorite::table
            .count()
            .filter(post_favorite::post_id.eq(post_id))
            .first(conn)?;
        let feature_count: i64 = post_feature::table
            .count()
            .filter(post_feature::post_id.eq(post_id))
            .first(conn)?;
        let last_comment_time: Option<DateTime> = comment::table
            .select(max(comment::creation_time))
            .filter(comment::post_id.eq(post_id))
            .first(conn)?;
        let last_favorite_time: Option<DateTime> = post_favorite::table
            .select(max(post_favorite::time))
            .filter(post_favorite::post_id.eq(post_id))
            .first(conn)?;
        let last_feature_time: Option<DateTime> = post_feature::table
            .select(max(post_feature::time))
            .filter(post_feature::post_id.eq(post_id))
            .first(conn)?;
        diesel::update(post_statistics::table)
            .set((
                post_statistics::tag_count.eq(tag_count as i32),
                post_statistics::pool_count.eq(pool_count as i32),
                post_statistics::note_count.eq(note_count as i32),
                post_statistics::comment_count.eq(comment_count as i32),
                post_statistics::relation_count.eq(relation_count as i32),
                post_statistics::score.eq(score.unwrap_or(0) as i32),
                post_statistics::favorite_count.eq(favorite_count as i32),
                post_statistics::feature_count.eq(feature_count as i32),
                post_statistics::last_comment_time.eq(last_comment_time),
                post_statistics::last_favorite_time.eq(last_favorite_time),
                post_statistics::last_feature_time.eq(last_feature_time),
            ))
            .filter(post_statistics::post_id.eq(post_id))
            .execute(conn)?;
    }

    Ok(())
}