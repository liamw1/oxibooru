use crate::admin::{DatabaseResult, LoopState, PRINT_INTERVAL, ProgressReporter};
use crate::content::hash::PostHash;
use crate::content::signature::SIGNATURE_VERSION;
use crate::content::thumbnail::{ThumbnailCategory, ThumbnailType};
use crate::content::{FileContents, decode, hash, signature, thumbnail};
use crate::model::post::{CompressedSignature, NewPostSignature};
use crate::schema::{database_statistics, post, post_signature};
use crate::time::Timer;
use crate::{admin, db, filesystem};
use diesel::dsl::exists;
use diesel::prelude::*;
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
        .try_for_each(|post_id| recompute_checksum(post_id, &progress, &duplicate_count))?;
    duplicate_count.report();
    Ok(())
}

/// Recomputes both post signatures and signature indexes.
/// Useful when the post signature parameters change.
pub fn recompute_signatures(conn: &mut PgConnection) -> DatabaseResult<()> {
    let _timer = Timer::new("recompute_signatures");
    let progress = ProgressReporter::new("Signatures computed", PRINT_INTERVAL);

    let post_ids: Vec<_> = post::table.select(post::id).load(conn)?;

    // Update signature version only after a successful data retrieval
    diesel::update(database_statistics::table)
        .set(database_statistics::signature_version.eq(SIGNATURE_VERSION))
        .execute(conn)?;

    post_ids
        .into_par_iter()
        .try_for_each(|post_id| recompute_signature(post_id, &progress))
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
    post_signatures.into_par_iter().try_for_each(|(post_id, signature)| {
        let indexes = signature::generate_indexes(&signature);
        diesel::update(post_signature::table.find(post_id))
            .set(post_signature::words.eq(indexes.as_slice()))
            .execute(&mut db::get_connection()?)?;
        progress.increment();
        Ok(())
    })
}

/// This functions prompts the user for input again to regenerate specific thumbnails.
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
        filesystem::save_post_thumbnail(&post_hash, thumbnail, ThumbnailCategory::Generated)
            .map_err(|err| format!("Cannot save thumbnail for post {post_id} for reason: {err}"))?;

        println!("Thumbnail regeneration successful.\n");
        Ok(LoopState::Continue)
    });
}

/// Recomputes checksum for post with id `post_id`.
pub fn recompute_checksum(
    post_id: i64,
    progress: &ProgressReporter,
    duplicate_count: &ProgressReporter,
) -> DatabaseResult<()> {
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
    let duplicate: Option<i64> = post::table
        .select(post::id)
        .filter(post::checksum.eq(&checksum))
        .filter(post::id.ne(post_id))
        .first(&mut conn)
        .optional()?;
    if let Some(dup_id) = duplicate {
        warn!("Potential duplicate post {dup_id} for post {post_id}");
        duplicate_count.increment();
        return Ok(());
    }

    diesel::update(post::table.find(post_id))
        .set((post::checksum.eq(checksum), post::checksum_md5.eq(md5_checksum)))
        .execute(&mut conn)?;
    progress.increment();
    Ok(())
}

/// Recomputes signature for post with id `post_id`.
fn recompute_signature(post_id: i64, progress: &ProgressReporter) -> DatabaseResult<()> {
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
    conn.transaction(|conn| {
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
            let new_post_signature = NewPostSignature {
                post_id,
                signature: image_signature.into(),
                words: signature_indexes.into(),
            };
            diesel::insert_into(post_signature::table)
                .values(new_post_signature)
                .execute(conn)
        }
    })?;
    progress.increment();
    Ok(())
}
