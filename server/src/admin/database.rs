use crate::admin::{DatabaseResult, PRINT_INTERVAL, ProgressReporter};
use crate::app::AppState;
use crate::content::hash::PostHash;
use crate::filesystem::Directory;
use crate::model::enums::MimeType;
use crate::schema::{
    comment, comment_score, comment_statistics, database_statistics, pool, pool_category, pool_category_statistics,
    pool_post, pool_statistics, post, post_favorite, post_feature, post_note, post_relation, post_score,
    post_statistics, post_tag, tag, tag_category, tag_category_statistics, tag_implication, tag_statistics,
    tag_suggestion, user, user_statistics,
};
use crate::time::{DateTime, Timer};
use crate::{admin, filesystem};
use diesel::dsl::{count, max, sum};
use diesel::{ExpressionMethods, NullableExpressionMethods, QueryDsl, RunQueryDsl};
use std::ffi::OsStr;
use tracing::{error, warn};

/// Renames post files and thumbnails.
/// Useful when the content hash changes.
pub fn reset_filenames(state: &AppState) -> std::io::Result<()> {
    let _timer = Timer::new("reset_filenames");
    if state.config.path(Directory::GeneratedThumbnails).try_exists()? {
        let progress = ProgressReporter::new("Generated thumbnails renamed", PRINT_INTERVAL);
        for entry in std::fs::read_dir(state.config.path(Directory::GeneratedThumbnails))? {
            let path = entry?.path();
            let Some(post_id) = admin::get_post_id(&path) else {
                error!("Could not find post_id of {path:?}");
                continue;
            };

            let new_path = PostHash::new(&state.config, post_id).generated_thumbnail_path();
            if path != new_path {
                filesystem::move_file(&path, &new_path)?;
                progress.increment();
            }
        }
    }
    if state.config.path(Directory::CustomThumbnails).try_exists()? {
        let progress = ProgressReporter::new("Custom thumbnails renamed", PRINT_INTERVAL);
        for entry in std::fs::read_dir(state.config.path(Directory::CustomThumbnails))? {
            let path = entry?.path();
            let Some(post_id) = admin::get_post_id(&path) else {
                error!("Could not find post_id of {path:?}");
                continue;
            };

            let new_path = PostHash::new(&state.config, post_id).custom_thumbnail_path();
            if path != new_path {
                filesystem::move_file(&path, &new_path)?;
                progress.increment();
            }
        }
    }
    if state.config.path(Directory::Posts).try_exists()? {
        let progress = ProgressReporter::new("Posts renamed", PRINT_INTERVAL);
        for entry in std::fs::read_dir(state.config.path(Directory::Posts))? {
            let path = entry?.path();
            let Some(post_id) = admin::get_post_id(&path) else {
                error!("Could not find post_id of {path:?}");
                continue;
            };

            let new_path = if let Some(mime_type) = MimeType::from_path(&path) {
                PostHash::new(&state.config, post_id).content_path(mime_type)
            } else {
                if let Some(extension) = path.extension().map(OsStr::to_string_lossy) {
                    warn!("Post {post_id} has unsupported file extension {extension}");
                } else {
                    warn!("Post {post_id} has no file extension");
                }

                let mut new_path = PostHash::new(&state.config, post_id).content_path(MimeType::Png);
                new_path.set_extension(path.extension().unwrap_or(OsStr::new("")));
                new_path
            };

            if path != new_path {
                filesystem::move_file(&path, &new_path)?;
                progress.increment();
            }
        }
    }
    Ok(())
}

/// Updates database values for thumbnail size.
pub fn reset_thumbnail_sizes(state: &AppState) -> DatabaseResult<()> {
    let mut conn = state.get_connection()?;

    if state.config.path(Directory::Avatars).try_exists()? {
        let progress = ProgressReporter::new("Avatar sizes cached", PRINT_INTERVAL);
        for entry in std::fs::read_dir(state.config.path(Directory::Avatars))? {
            let path = entry?.path();
            let Some(username) = path.file_name().map(OsStr::to_string_lossy) else {
                error!("Unable to convert file name of {path:?} to string");
                continue;
            };

            let file_size = filesystem::file_size(&path)?;
            diesel::update(user::table)
                .set(user::custom_avatar_size.eq(file_size))
                .filter(user::name.eq(username))
                .execute(&mut conn)?;
            progress.increment();
        }
    }
    if state.config.path(Directory::GeneratedThumbnails).try_exists()? {
        let progress = ProgressReporter::new("Generated thumbnail sizes cached", PRINT_INTERVAL);
        for entry in std::fs::read_dir(state.config.path(Directory::GeneratedThumbnails))? {
            let path = entry?.path();
            let Some(post_id) = admin::get_post_id(&path) else {
                error!("Could not find post_id of {path:?}");
                continue;
            };

            let file_size = filesystem::file_size(&path)?;
            diesel::update(post::table)
                .set(post::generated_thumbnail_size.eq(file_size))
                .filter(post::id.eq(post_id))
                .execute(&mut conn)?;
            progress.increment();
        }
    }
    if state.config.path(Directory::CustomThumbnails).try_exists()? {
        let progress = ProgressReporter::new("Custom thumbnails sizes cached", PRINT_INTERVAL);
        for entry in std::fs::read_dir(state.config.path(Directory::CustomThumbnails))? {
            let path = entry?.path();
            let Some(post_id) = admin::get_post_id(&path) else {
                error!("Could not find post_id of {path:?}");
                continue;
            };

            let file_size = filesystem::file_size(&path)?;
            diesel::update(post::table)
                .set(post::custom_thumbnail_size.eq(file_size))
                .filter(post::id.eq(post_id))
                .execute(&mut conn)?;
            progress.increment();
        }
    }
    Ok(())
}

/// Recomputes database table statistics. Useful for when new statistics are added
/// or a bug is found in statistics updaters.
///
/// Because it computes statistics one row at a time, this function is fairly slow.
/// A much faster version of this is done in `scripts/convert_szuru_database.sql`,
/// but it would be very tricky to implement in Diesel.
pub fn reset_relation_stats(state: &AppState) -> DatabaseResult<()> {
    let mut conn = state.get_connection()?;

    let comment_count: i64 = comment::table.count().first(&mut conn)?;
    let pool_count: i64 = pool::table.count().first(&mut conn)?;
    let post_count: i64 = post::table.count().first(&mut conn)?;
    let tag_count: i64 = tag::table.count().first(&mut conn)?;
    let user_count: i64 = user::table.count().first(&mut conn)?;
    diesel::update(database_statistics::table)
        .set((
            database_statistics::comment_count.eq(comment_count),
            database_statistics::pool_count.eq(pool_count),
            database_statistics::post_count.eq(post_count),
            database_statistics::tag_count.eq(tag_count),
            database_statistics::user_count.eq(user_count),
        ))
        .execute(&mut conn)?;

    let comment_stats: Vec<(i64, Option<i64>)> = comment::table
        .left_join(comment_score::table)
        .group_by(comment::id)
        .select((comment::id, sum(comment_score::score).nullable()))
        .load(&mut conn)?;
    let progress = ProgressReporter::new("Comment statistics calculated", PRINT_INTERVAL);
    for (comment_id, score) in comment_stats {
        diesel::update(comment_statistics::table.find(comment_id))
            .set(comment_statistics::score.eq(score.unwrap_or(0)))
            .execute(&mut conn)?;
        progress.increment();
    }

    let pool_category_stats: Vec<(i64, Option<i64>)> = pool_category::table
        .left_join(pool::table)
        .group_by(pool_category::id)
        .select((pool_category::id, count(pool::id).nullable()))
        .load(&mut conn)?;
    let progress = ProgressReporter::new("Pool category statistics calculated", PRINT_INTERVAL);
    for (category_id, usage_count) in pool_category_stats {
        diesel::update(pool_category_statistics::table.find(category_id))
            .set(pool_category_statistics::usage_count.eq(usage_count.unwrap_or(0)))
            .execute(&mut conn)?;
        progress.increment();
    }

    let pool_stats: Vec<(i64, Option<i64>)> = pool::table
        .left_join(pool_post::table)
        .group_by(pool::id)
        .select((pool::id, count(pool_post::post_id).nullable()))
        .load(&mut conn)?;
    let progress = ProgressReporter::new("Pool statistics calculated", PRINT_INTERVAL);
    for (pool_id, post_count) in pool_stats {
        diesel::update(pool_statistics::table.find(pool_id))
            .set(pool_statistics::post_count.eq(post_count.unwrap_or(0)))
            .execute(&mut conn)?;
        progress.increment();
    }

    let tag_category_stats: Vec<(i64, Option<i64>)> = tag_category::table
        .left_join(tag::table)
        .group_by(tag_category::id)
        .select((tag_category::id, count(tag::id).nullable()))
        .load(&mut conn)?;
    let progress = ProgressReporter::new("Tag category statistics calculated", PRINT_INTERVAL);
    for (category_id, usage_count) in tag_category_stats {
        diesel::update(tag_category_statistics::table.find(category_id))
            .set(tag_category_statistics::usage_count.eq(usage_count.unwrap_or(0)))
            .execute(&mut conn)?;
        progress.increment();
    }

    let tag_ids: Vec<i64> = tag::table.select(tag::id).load(&mut conn)?;
    let progress = ProgressReporter::new("Tag statistics calculated", PRINT_INTERVAL);
    for tag_id in tag_ids {
        let usage_count: i64 = post_tag::table
            .filter(post_tag::tag_id.eq(tag_id))
            .count()
            .first(&mut conn)?;
        let implication_count: i64 = tag_implication::table
            .filter(tag_implication::child_id.eq(tag_id))
            .count()
            .first(&mut conn)?;
        let suggestion_count: i64 = tag_suggestion::table
            .filter(tag_suggestion::child_id.eq(tag_id))
            .count()
            .first(&mut conn)?;
        diesel::update(tag_statistics::table.find(tag_id))
            .set((
                tag_statistics::usage_count.eq(usage_count),
                tag_statistics::implication_count.eq(implication_count),
                tag_statistics::suggestion_count.eq(suggestion_count),
            ))
            .execute(&mut conn)?;
        progress.increment();
    }

    let user_ids: Vec<i64> = user::table.select(user::id).load(&mut conn)?;
    let progress = ProgressReporter::new("User statistics calculated", PRINT_INTERVAL);
    for user_id in user_ids {
        let comment_count: i64 = comment::table
            .filter(comment::user_id.eq(user_id))
            .count()
            .first(&mut conn)?;
        let favorite_count: i64 = post_favorite::table
            .filter(post_favorite::user_id.eq(user_id))
            .count()
            .first(&mut conn)?;
        let upload_count: i64 = post::table.filter(post::user_id.eq(user_id)).count().first(&mut conn)?;
        diesel::update(user_statistics::table.find(user_id))
            .set((
                user_statistics::comment_count.eq(comment_count),
                user_statistics::favorite_count.eq(favorite_count),
                user_statistics::upload_count.eq(upload_count),
            ))
            .execute(&mut conn)?;
        progress.increment();
    }

    let post_ids: Vec<i64> = post::table.select(post::id).load(&mut conn)?;
    let progress = ProgressReporter::new("Post statistics calculated", PRINT_INTERVAL);
    for post_id in post_ids {
        let tag_count: i64 = post_tag::table
            .filter(post_tag::post_id.eq(post_id))
            .count()
            .first(&mut conn)?;
        let pool_count: i64 = pool_post::table
            .filter(pool_post::post_id.eq(post_id))
            .count()
            .first(&mut conn)?;
        let note_count: i64 = post_note::table
            .filter(post_note::post_id.eq(post_id))
            .count()
            .first(&mut conn)?;
        let comment_count: i64 = comment::table
            .filter(comment::post_id.eq(post_id))
            .count()
            .first(&mut conn)?;
        let relation_count: i64 = post_relation::table
            .filter(post_relation::child_id.eq(post_id))
            .count()
            .first(&mut conn)?;
        let score: Option<i64> = post_score::table
            .select(sum(post_score::score))
            .filter(post_score::post_id.eq(post_id))
            .first(&mut conn)?;
        let favorite_count: i64 = post_favorite::table
            .filter(post_favorite::post_id.eq(post_id))
            .count()
            .first(&mut conn)?;
        let feature_count: i64 = post_feature::table
            .filter(post_feature::post_id.eq(post_id))
            .count()
            .first(&mut conn)?;
        let last_comment_time: Option<DateTime> = comment::table
            .select(max(comment::creation_time))
            .filter(comment::post_id.eq(post_id))
            .first(&mut conn)?;
        let last_favorite_time: Option<DateTime> = post_favorite::table
            .select(max(post_favorite::time))
            .filter(post_favorite::post_id.eq(post_id))
            .first(&mut conn)?;
        let last_feature_time: Option<DateTime> = post_feature::table
            .select(max(post_feature::time))
            .filter(post_feature::post_id.eq(post_id))
            .first(&mut conn)?;
        diesel::update(post_statistics::table.find(post_id))
            .set((
                post_statistics::tag_count.eq(tag_count),
                post_statistics::pool_count.eq(pool_count),
                post_statistics::note_count.eq(note_count),
                post_statistics::comment_count.eq(comment_count),
                post_statistics::relation_count.eq(relation_count),
                post_statistics::score.eq(score.unwrap_or(0)),
                post_statistics::favorite_count.eq(favorite_count),
                post_statistics::feature_count.eq(feature_count),
                post_statistics::last_comment_time.eq(last_comment_time),
                post_statistics::last_favorite_time.eq(last_favorite_time),
                post_statistics::last_feature_time.eq(last_feature_time),
            ))
            .execute(&mut conn)?;
        progress.increment();
    }
    Ok(())
}

/// Recalculates cached file sizes, row counts, and table statistics.
/// Useful for when the statistics become inconsistent with database
/// or when migrating from an older version without statistics.
pub fn reset_statistics(state: &AppState) -> DatabaseResult<()> {
    let _timer = Timer::new("reset_statistics");

    // Disk usage will automatically be incremented via triggers as we calculate
    // content, thumbnail, and avatar sizes
    let mut conn = state.get_connection()?;
    diesel::update(database_statistics::table)
        .set(database_statistics::disk_usage.eq(0))
        .execute(&mut conn)?;

    if state.config.path(Directory::Posts).try_exists()? {
        let progress = ProgressReporter::new("Posts content sizes cached", PRINT_INTERVAL);
        for entry in std::fs::read_dir(state.config.path(Directory::Posts))? {
            let path = entry?.path();
            let Some(post_id) = admin::get_post_id(&path) else {
                error!("Could not find post_id of {path:?}");
                continue;
            };

            let file_size = filesystem::file_size(&path)?;
            diesel::update(post::table)
                .set(post::file_size.eq(file_size))
                .filter(post::id.eq(post_id))
                .execute(&mut conn)?;
            progress.increment();
        }
    }
    reset_thumbnail_sizes(state)?;
    reset_relation_stats(state)
}
