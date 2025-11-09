use crate::admin::{DatabaseError, DatabaseResult, LoopState, PRINT_INTERVAL, ProgressReporter};
use crate::content::hash::PostHash;
use crate::content::signature::SIGNATURE_VERSION;
use crate::content::thumbnail::{ThumbnailCategory, ThumbnailType};
use crate::content::{FileContents, decode, hash, signature, thumbnail};
use crate::model::enums::MimeType;
use crate::model::post::{CompressedSignature, NewPostSignature};
use crate::schema::{database_statistics, post, post_signature};
use crate::time::Timer;
use crate::{admin, db, update};
use diesel::dsl::exists;
use diesel::prelude::*;
use diesel::r2d2::PoolError;
use rayon::prelude::*;
use tracing::{error, warn};

/// Recomputes posts checksums.
/// Useful when the way we compute checksums changes.
pub fn recompute_checksums(conn: &mut PgConnection) -> DatabaseResult<()> {
    let _timer = Timer::new("recompute_checksums");
    let progress = ProgressReporter::new("Checksums computed", PRINT_INTERVAL);
    let duplicate_count = ProgressReporter::new("Duplicates found", PRINT_INTERVAL);

    let post_ids: Vec<_> = post::table.select(post::id).load(conn)?;
    post_ids
        .into_par_iter()
        .try_for_each(|post_id| recompute_checksum_in_parallel(post_id, &progress, &duplicate_count))
        .map_err(DatabaseError::from)?;
    duplicate_count.report();
    Ok(())
}

/// Recomputes both post signatures and signature indexes.
/// Useful when the post signature parameters change.
pub fn recompute_signatures(conn: &mut PgConnection) -> DatabaseResult<()> {
    let _timer = Timer::new("recompute_signatures");
    let progress = ProgressReporter::new("Signatures computed", PRINT_INTERVAL);

    let post_ids: Vec<_> = post::table.select(post::id).load(conn)?;

    // Update signature version only after a successful data retrieval.
    // We do this before actually recomputing signatures so that server
    // can continue running during computation.
    diesel::update(database_statistics::table)
        .set(database_statistics::signature_version.eq(SIGNATURE_VERSION))
        .execute(conn)?;

    post_ids
        .into_par_iter()
        .try_for_each(|post_id| recompute_signature_in_parallel(post_id, &progress))
        .map_err(DatabaseError::from)
}

/// Recomputes post signature indexes.
/// Useful when the post signature index parameters change.
///
/// This is much faster than recomputing the signatures, as this function doesn't require
/// reading post content from disk.
pub fn recompute_indexes(conn: &mut PgConnection) -> DatabaseResult<()> {
    let _timer = Timer::new("recompute_indexes");
    let progress = ProgressReporter::new("Indexes computed", PRINT_INTERVAL);

    let post_signatures: Vec<(i64, CompressedSignature)> = post_signature::table
        .select((post_signature::post_id, post_signature::signature))
        .load(conn)?;
    post_signatures
        .into_par_iter()
        .try_for_each(|(post_id, signature)| recompute_index_in_parallel(post_id, &signature, &progress))
        .map_err(DatabaseError::from)
}

pub fn regenerate_thumbnails(conn: &mut PgConnection) -> DatabaseResult<()> {
    let _timer = Timer::new("regenerate_thumbnails");
    let progress = ProgressReporter::new("Thumbnails regenerated", PRINT_INTERVAL);

    let post_ids: Vec<_> = post::table.select((post::id, post::mime_type)).load(conn)?;
    post_ids
        .into_par_iter()
        .try_for_each(|(post_id, mime_type)| regenerate_thumbnail_in_parallel(post_id, mime_type, &progress))
        .map_err(DatabaseError::from)
}

/// Prompts the user for input again to regenerate specific thumbnails.
pub fn regenerate_thumbnail(conn: &mut PgConnection) {
    admin::user_input_loop(conn, |conn: &mut PgConnection, buffer: &mut String| {
        println!("Please enter the post ID you would like to generate a thumbnail for. Enter \"done\" when finished.");
        let user_input = admin::prompt_user_input("Post ID", buffer);
        if let Ok(state) = LoopState::try_from(user_input) {
            return Ok(state);
        }

        let post_id = user_input
            .parse::<i64>()
            .map_err(|_| String::from("Post ID must be an integer"))?;
        let mime_type = post::table
            .find(post_id)
            .select(post::mime_type)
            .first(conn)
            .map_err(|err| format!("Cannot retrieve MIME type for post {post_id} for reason: {err}"))?;

        let post_hash = PostHash::new(post_id);
        let content_path = post_hash.content_path(mime_type);
        let data = std::fs::read(&content_path)
            .map_err(|err| format!("Cannot read content for post {post_id} for reason: {err}"))?;

        let file_contents = FileContents { data, mime_type };
        let thumbnail = decode::representative_image(&file_contents, &content_path)
            .map(|image| thumbnail::create(&image, ThumbnailType::Post))
            .map_err(|err| format!("Cannot decode content for post {post_id} for reason: {err}"))?;
        update::post::thumbnail(conn, &post_hash, &thumbnail, ThumbnailCategory::Generated)
            .map_err(|err| format!("Cannot save thumbnail for post {post_id} for reason: {err}"))?;

        println!("Thumbnail regeneration successful.\n");
        Ok(LoopState::Continue)
    });
}

/// Recomputes index for post with id `post_id`. Designed to operate in a parallel iterator.
fn recompute_index_in_parallel(
    post_id: i64,
    signature: &CompressedSignature,
    progress: &ProgressReporter,
) -> Result<(), PoolError> {
    let mut conn = db::get_connection()?;
    let indexes = signature::generate_indexes(signature);
    match diesel::update(post_signature::table.find(post_id))
        .set(post_signature::words.eq(indexes.as_slice()))
        .execute(&mut conn)
    {
        Ok(_) => progress.increment(),
        Err(err) => error!("Index update failed for post {post_id} for reason: {err}"),
    }
    Ok(())
}

/// Recomputes checksum for post with id `post_id`. Designed to operate in a parallel iterator.
fn recompute_checksum_in_parallel(
    post_id: i64,
    progress: &ProgressReporter,
    duplicate_count: &ProgressReporter,
) -> Result<(), PoolError> {
    let mut conn = db::get_connection()?;
    let mime_type = match post::table.find(post_id).select(post::mime_type).first(&mut conn) {
        Ok(mime_type) => mime_type,
        Err(err) => {
            error!("Cannot retrieve MIME type for post {post_id} for reason: {err}");
            return Ok(());
        }
    };

    let image_path = PostHash::new(post_id).content_path(mime_type);
    let file_contents = match std::fs::read(&image_path) {
        Ok(contents) => contents,
        Err(err) => {
            error!("Unable to compute checksum for post {post_id} for reason: {err}");
            return Ok(());
        }
    };

    let checksum = hash::compute_checksum(&file_contents);
    let md5_checksum = hash::compute_md5_checksum(&file_contents);
    let duplicate: Option<i64> = match post::table
        .select(post::id)
        .filter(post::checksum.eq(&checksum))
        .filter(post::id.ne(post_id))
        .first(&mut conn)
        .optional()
    {
        Ok(dup) => dup,
        Err(err) => {
            error!("Duplicate check failed for post {post_id} for reason: {err}");
            return Ok(());
        }
    };
    if let Some(dup_id) = duplicate {
        warn!("Potential duplicate post {dup_id} for post {post_id}");
        duplicate_count.increment();
        return Ok(());
    }

    match diesel::update(post::table.find(post_id))
        .set((post::checksum.eq(checksum), post::checksum_md5.eq(md5_checksum)))
        .execute(&mut conn)
    {
        Ok(_) => progress.increment(),
        Err(err) => error!("Checksum update failed for post {post_id} for reason: {err}"),
    }
    Ok(())
}

/// Recomputes signature for post with id `post_id`. Designed to operate in a parallel iterator.
fn recompute_signature_in_parallel(post_id: i64, progress: &ProgressReporter) -> Result<(), PoolError> {
    let mut conn = db::get_connection()?;
    let mime_type = match post::table
        .find(post_id)
        .select(post::mime_type)
        .first(&mut conn)
        .optional()
    {
        Ok(Some(mime_type)) => mime_type,
        Ok(None) => return Ok(()), // Post must have been deleted after starting task, skip
        Err(err) => {
            error!("Cannot retrieve MIME type for post {post_id} for reason: {err}");
            return Ok(());
        }
    };

    let image_path = PostHash::new(post_id).content_path(mime_type);
    let data = match std::fs::read(&image_path) {
        Ok(contents) => contents,
        Err(err) => {
            error!("Unable to read file for post {post_id} for reason: {err}");
            return Ok(());
        }
    };

    let file_contents = FileContents { data, mime_type };
    let image = match decode::representative_image(&file_contents, &image_path) {
        Ok(image) => image,
        Err(err) => {
            error!("Unable to get representative image for post {post_id} for reason: {err}");
            return Ok(());
        }
    };

    let image_signature = signature::compute(&image);
    let signature_indexes = signature::generate_indexes(&image_signature);
    let transaction_result = conn.transaction(|conn| {
        // Post may have been deleted, so make sure it still exists first
        let post_exists: bool = diesel::select(exists(post::table.find(post_id))).get_result(conn)?;
        if !post_exists {
            return Ok(0);
        }

        let signature_exists: bool = diesel::select(exists(post_signature::table.find(post_id))).get_result(conn)?;
        if signature_exists {
            diesel::update(post_signature::table.find(post_id))
                .set((
                    post_signature::signature.eq(image_signature.as_slice()),
                    post_signature::words.eq(signature_indexes.as_slice()),
                ))
                .execute(conn)
        } else {
            NewPostSignature {
                post_id,
                signature: image_signature.into(),
                words: signature_indexes.into(),
            }
            .insert_into(post_signature::table)
            .execute(conn)
        }
    });

    match transaction_result {
        Ok(_) => progress.increment(),
        Err(err) => error!("Unable to update post signature for post {post_id} for reason: {err}"),
    }
    Ok(())
}

/// Regenerates thumbnail for post with id `post_id`. Designed to operate in a parallel iterator.
fn regenerate_thumbnail_in_parallel(
    post_id: i64,
    mime_type: MimeType,
    progress: &ProgressReporter,
) -> Result<(), PoolError> {
    let mut conn = db::get_connection()?;

    let post_hash = PostHash::new(post_id);
    let content_path = post_hash.content_path(mime_type);
    let data = match std::fs::read(&content_path) {
        Ok(data) => data,
        Err(err) => {
            error!("Cannot read content for post {post_id} for reason: {err}");
            return Ok(());
        }
    };

    let file_contents = FileContents { data, mime_type };
    let thumbnail = match decode::representative_image(&file_contents, &content_path) {
        Ok(image) => thumbnail::create(&image, ThumbnailType::Post),
        Err(err) => {
            error!("Cannot decode content for post {post_id} for reason: {err}");
            return Ok(());
        }
    };
    if let Err(err) = update::post::thumbnail(&mut conn, &post_hash, &thumbnail, ThumbnailCategory::Generated) {
        error!("Cannot save thumbnail for post {post_id} for reason: {err}");
    } else {
        progress.increment();
    }
    Ok(())
}
