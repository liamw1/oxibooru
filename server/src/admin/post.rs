use crate::admin::ProgressReporter;
use crate::admin::PRINT_INTERVAL;
use crate::api::ApiResult;
use crate::content::hash::PostHash;
use crate::content::thumbnail::{ThumbnailCategory, ThumbnailType};
use crate::content::{decode, hash, signature, thumbnail, FileContents};
use crate::model::post::{NewPostSignature, PostSignature};
use crate::schema::{post, post_signature};
use crate::time::Timer;
use crate::{admin, filesystem};
use diesel::dsl::exists;
use diesel::prelude::*;

/// Recomputes posts checksums.
/// Useful when the way we compute checksums changes.
pub fn recompute_checksums(conn: &mut PgConnection) -> QueryResult<()> {
    let _timer = Timer::new("recompute_checksums");
    let mut progress = ProgressReporter::new("Checksums computed", PRINT_INTERVAL);
    let mut duplicate_count = ProgressReporter::new("Duplicates found", PRINT_INTERVAL);

    let post_ids: Vec<_> = post::table.select(post::id).load(conn)?;
    for post_id in post_ids.into_iter() {
        let mime_type = match post::table.find(post_id).select(post::mime_type).first(conn) {
            Ok(mime_type) => mime_type,
            Err(err) => {
                eprintln!("ERROR: Cannot retrieve MIME type for post {post_id} for reason: {err}");
                continue;
            }
        };

        let image_path = PostHash::new(post_id).content_path(mime_type);
        let file_contents = match std::fs::read(&image_path) {
            Ok(contents) => contents,
            Err(err) => {
                eprintln!("ERROR: Unable to compute checksum for post {post_id} for reason: {err}");
                continue;
            }
        };

        let checksum = hash::compute_checksum(&file_contents);
        let md5_checksum = hash::compute_md5_checksum(&file_contents);
        let duplicate: Option<i64> = post::table
            .select(post::id)
            .filter(post::checksum.eq(&checksum))
            .filter(post::id.ne(post_id))
            .first(conn)
            .optional()?;
        if let Some(dup_id) = duplicate {
            eprintln!("WARNING: Potential duplicate post {dup_id} for post {post_id}");
            duplicate_count.increment();
            continue;
        }

        diesel::update(post::table.find(post_id))
            .set((post::checksum.eq(checksum), post::checksum_md5.eq(md5_checksum)))
            .execute(conn)?;
        progress.increment();
    }
    duplicate_count.report();
    Ok(())
}

/// Recomputes both post signatures and signature indexes.
/// Useful when the post signature parameters change.
///
/// This function is quite slow for large databases.
/// I'll look into parallelizing this in the future.
pub fn recompute_signatures(conn: &mut PgConnection) -> QueryResult<()> {
    let _timer = Timer::new("recompute_signatures");
    let mut progress = ProgressReporter::new("Signatures computed", PRINT_INTERVAL);

    let post_ids: Vec<_> = post::table.select(post::id).load(conn)?;
    for post_id in post_ids.into_iter() {
        let mime_type = match post::table.find(post_id).select(post::mime_type).first(conn) {
            Ok(mime_type) => mime_type,
            Err(err) => {
                eprintln!("ERROR: Cannot retrieve MIME type for post {post_id} for reason: {err}");
                continue;
            }
        };

        let image_path = PostHash::new(post_id).content_path(mime_type);
        let data = match std::fs::read(&image_path) {
            Ok(contents) => contents,
            Err(err) => {
                eprintln!("ERROR: Unable to read file for post {post_id} for reason: {err}");
                continue;
            }
        };

        let file_contents = FileContents { data, mime_type };
        let image = match decode::representative_image(&file_contents, &image_path) {
            Ok(image) => image,
            Err(err) => {
                eprintln!("ERROR: Unable to get representative image for post {post_id} for reason: {err}");
                continue;
            }
        };

        let image_signature = signature::compute(&image);
        let signature_indexes = signature::generate_indexes(image_signature);
        let signature_exists: bool = diesel::select(exists(post_signature::table.find(post_id))).get_result(conn)?;

        if signature_exists {
            diesel::update(post_signature::table.find(post_id))
                .set((
                    post_signature::signature.eq(image_signature.as_slice()),
                    post_signature::words.eq(signature_indexes.as_slice()),
                ))
                .execute(conn)?;
        } else {
            let new_post_signature = NewPostSignature {
                post_id,
                signature: &image_signature,
                words: &signature_indexes,
            };
            diesel::insert_into(post_signature::table)
                .values(new_post_signature)
                .execute(conn)?;
        }
        progress.increment();
    }
    Ok(())
}

/// Recomputes post signature indexes.
/// Useful when the post signature index parameters change.
///
/// This is much faster than recomputing the signatures, as this function doesn't require
/// reading post content from disk.
pub fn recompute_indexes(conn: &mut PgConnection) -> QueryResult<()> {
    let _timer = Timer::new("recompute_indexes");

    conn.transaction(|conn| {
        let post_signatures: Vec<_> = post_signature::table.select(PostSignature::as_select()).load(conn)?;
        let (post_ids, converted_signatures): (Vec<_>, Vec<_>) = post_signatures
            .into_iter()
            .map(|post_sig| (post_sig.post_id, signature::from_database(post_sig.signature)))
            .unzip();
        let indexes: Vec<_> = converted_signatures
            .iter()
            .copied()
            .map(signature::generate_indexes)
            .collect();
        let new_post_signatures: Vec<_> = post_ids
            .iter()
            .zip(converted_signatures.iter())
            .zip(indexes.iter())
            .map(|((&post_id, signature), words)| NewPostSignature {
                post_id,
                signature,
                words,
            })
            .collect();

        diesel::delete(post_signature::table).execute(conn)?;

        // Postgres has a limit on the number of parameters that can be in a query, so
        // we batch the insertion of post signatures in chunks.
        const SIGNATURE_BATCH_SIZE: usize = 10000;
        for (chunk_index, post_signature_chunk) in new_post_signatures.chunks(SIGNATURE_BATCH_SIZE).enumerate() {
            diesel::insert_into(post_signature::table)
                .values(post_signature_chunk)
                .execute(conn)?;
            println!("Indexes computed: {}", (chunk_index + 1) * SIGNATURE_BATCH_SIZE);
        }
        Ok(())
    })
}

/// This functions prompts the user for input again to regenerate specific thumbnails.
pub fn regenerate_thumbnail(conn: &mut PgConnection) -> ApiResult<()> {
    let mut buffer = String::new();
    loop {
        println!("Please enter the post ID you would like to generate a thumbnail for. Enter \"done\" when finished.");
        let user_input = admin::prompt_user_input("Post ID", &mut buffer);
        if user_input == "done" {
            break;
        }

        let post_id = match user_input.parse::<i64>() {
            Ok(id) => id,
            Err(_) => {
                eprintln!("ERROR: Post ID must be an integer\n");
                continue;
            }
        };

        let mime_type = match post::table.find(post_id).select(post::mime_type).first(conn) {
            Ok(mime_type) => mime_type,
            Err(err) => {
                eprintln!("ERROR: Cannot retrieve MIME type for post {post_id} for reason: {err}");
                continue;
            }
        };

        let post_hash = PostHash::new(post_id);
        let content_path = post_hash.content_path(mime_type);
        let data = std::fs::read(&content_path)?;

        let file_contents = FileContents { data, mime_type };
        let thumbnail = decode::representative_image(&file_contents, &content_path)
            .map(|image| thumbnail::create(&image, ThumbnailType::Post))?;
        filesystem::save_post_thumbnail(&post_hash, thumbnail, ThumbnailCategory::Generated)?;

        println!("Thumbnail regeneration successful.\n");
    }
    Ok(())
}
