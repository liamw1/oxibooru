use crate::admin::ProgressReporter;
use crate::api::ApiResult;
use crate::auth::password;
use crate::config::RegexType;
use crate::content::hash::PostHash;
use crate::content::thumbnail::{ThumbnailCategory, ThumbnailType};
use crate::content::{decode, hash, signature, thumbnail};
use crate::filesystem::Directory;
use crate::model::enums::MimeType;
use crate::model::post::{NewPostSignature, PostSignature};
use crate::schema::{post, post_signature, user};
use crate::time::Timer;
use crate::{admin, api, filesystem};
use argon2::password_hash::SaltString;
use diesel::dsl::exists;
use diesel::prelude::*;
use rand_core::OsRng;
use std::path::Path;

/// Renames post files and thumbnails.
/// Useful when the content hash changes.
pub fn reset_filenames() -> std::io::Result<()> {
    let _time = Timer::new("reset_filenames");

    if filesystem::path(Directory::GeneratedThumbnails).exists() {
        let mut progress = ProgressReporter::new("Generated thumbnails renamed", PRINT_INTERVAL);
        for entry in std::fs::read_dir(filesystem::path(Directory::GeneratedThumbnails))? {
            let path = entry?.path();
            if let Some(post_id) = get_post_id(&path) {
                let new_path = PostHash::new(post_id).generated_thumbnail_path();
                if path != new_path {
                    std::fs::rename(path, new_path)?;
                }
                progress.increment();
            } else {
                eprintln!("ERROR: Could not find post_id of {path:?}");
            }
        }
    }

    if filesystem::path(Directory::CustomThumbnails).exists() {
        let mut progress = ProgressReporter::new("Custom thumbnails renamed", PRINT_INTERVAL);
        for entry in std::fs::read_dir(filesystem::path(Directory::CustomThumbnails))? {
            let path = entry?.path();
            if let Some(post_id) = get_post_id(&path) {
                let new_path = PostHash::new(post_id).custom_thumbnail_path();
                if path != new_path {
                    std::fs::rename(path, new_path)?;
                }
                progress.increment();
            } else {
                eprintln!("ERROR: Could not find post_id of {path:?}");
            }
        }
    }

    if filesystem::path(Directory::Posts).exists() {
        let mut progress = ProgressReporter::new("Posts renamed", PRINT_INTERVAL);
        for entry in std::fs::read_dir(filesystem::path(Directory::Posts))? {
            let path = entry?.path();
            let post_id = get_post_id(&path);
            let content_type = MimeType::from_path(&path);
            if let (Some(id), Some(mime_type)) = (post_id, content_type) {
                let new_path = PostHash::new(id).content_path(mime_type);
                if path != new_path {
                    std::fs::rename(path, new_path)?;
                }
                progress.increment();
            } else {
                eprintln!("ERROR: Could not find post_id or mime_type of {path:?}");
            }
        }
    }

    Ok(())
}

/// Recomputes posts checksums.
/// Useful when the way we compute checksums changes.
pub fn recompute_checksums(conn: &mut PgConnection) -> QueryResult<()> {
    let _time = Timer::new("recompute_checksums");
    let mut progress = ProgressReporter::new("Checksums computed", PRINT_INTERVAL);

    let posts: Vec<(i32, MimeType)> = post::table.select((post::id, post::mime_type)).load(conn)?;
    for (post_id, mime_type) in posts.into_iter() {
        let image_path = PostHash::new(post_id).content_path(mime_type);
        match std::fs::read(&image_path) {
            Ok(file_content) => {
                let checksum = hash::compute_checksum(&file_content);
                let md5_checksum = hash::compute_md5_checksum(&file_content);
                let duplicate: Option<i32> = post::table
                    .select(post::id)
                    .filter(post::checksum.eq(&checksum))
                    .filter(post::id.ne(post_id))
                    .first(conn)
                    .optional()?;
                if let Some(dup_id) = duplicate {
                    eprintln!("ERROR: Potential duplicate post {dup_id} for post {post_id}");
                } else {
                    diesel::update(post::table.find(post_id))
                        .set((post::checksum.eq(checksum), post::checksum_md5.eq(md5_checksum)))
                        .execute(conn)?;
                    progress.increment();
                }
            }
            Err(err) => eprintln!("ERROR: Unable to compute checksum for post {post_id} for reason: {err}"),
        }
    }

    Ok(())
}

/// Recomputes both post signatures and signature indexes.
/// Useful when the post signature parameters change.
///
/// This function is quite slow for large databases.
/// I'll look into parallelizing this in the future.
pub fn recompute_signatures(conn: &mut PgConnection) -> QueryResult<()> {
    let _time = Timer::new("recompute_signatures");
    let mut progress = ProgressReporter::new("Signatures computed", PRINT_INTERVAL);

    diesel::delete(post_signature::table).execute(conn)?;

    let posts: Vec<(i32, MimeType)> = post::table.select((post::id, post::mime_type)).load(conn)?;
    for (post_id, mime_type) in posts.into_iter() {
        let image_path = PostHash::new(post_id).content_path(mime_type);
        let file_content = match std::fs::read(&image_path) {
            Ok(content) => content,
            Err(err) => {
                eprintln!("ERROR: Unable to read file for post {post_id} for reason: {err}");
                continue;
            }
        };

        match decode::representative_image(&file_content, &image_path, mime_type) {
            Ok(image) => {
                let image_signature = signature::compute(&image);
                let signature_indexes = signature::generate_indexes(image_signature);
                let new_post_signature = NewPostSignature {
                    post_id,
                    signature: &image_signature,
                    words: &signature_indexes,
                };
                diesel::insert_into(post_signature::table)
                    .values(new_post_signature)
                    .execute(conn)?;
                progress.increment();
            }
            Err(err) => eprintln!("ERROR: Unable to compute signature for post {post_id} for reason: {err}"),
        }
    }

    Ok(())
}

/// Recomputes post signature indexes.
/// Useful when the post signature index parameters change.
///
/// This is much faster than recomputing the signatures, as this function doesn't require
/// reading post content from disk.
pub fn recompute_indexes(conn: &mut PgConnection) -> QueryResult<()> {
    let _time = Timer::new("recompute_indexes");

    conn.transaction(|conn| {
        let post_signatures: Vec<PostSignature> =
            post_signature::table.select(PostSignature::as_select()).load(conn)?;
        let converted_signatures: Vec<_> = post_signatures
            .into_iter()
            .map(|post_sig| (post_sig.post_id, signature::from_database(post_sig.signature)))
            .collect();
        let indexes: Vec<_> = converted_signatures
            .iter()
            .copied()
            .map(|(_, signature)| signature::generate_indexes(signature))
            .collect();
        let new_post_signatures: Vec<_> = converted_signatures
            .iter()
            .zip(indexes.iter())
            .map(|(sig, words)| NewPostSignature {
                post_id: sig.0,
                signature: &sig.1,
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

        if let Ok(post_id) = user_input.parse::<i32>() {
            let mime_type: MimeType = post::table.find(post_id).select(post::mime_type).first(conn)?;
            let post_hash = PostHash::new(post_id);
            let content_path = post_hash.content_path(mime_type);
            let file_contents = std::fs::read(&content_path)?;

            let thumbnail = decode::representative_image(&file_contents, &content_path, mime_type)
                .map(|image| thumbnail::create(&image, ThumbnailType::Post))?;
            filesystem::save_post_thumbnail(&post_hash, thumbnail, ThumbnailCategory::Generated)?;
        } else {
            eprintln!("ERROR: Post ID must be an integer\n");
            continue;
        }
        println!("Thumbnail regeneration successful.\n");
    }
    Ok(())
}

/// This function prompts the user for input again to reset passwords for specific users.
pub fn reset_password(conn: &mut PgConnection) -> ApiResult<()> {
    let mut user_buffer = String::new();
    let mut password_buffer = String::new();
    loop {
        println!("Please enter the username of the user you would like to reset a password for. Enter \"done\" when finished.");
        let user = admin::prompt_user_input("Username", &mut user_buffer);
        if user == "done" {
            break;
        }

        let user_exists: bool = diesel::select(exists(user::table.filter(user::name.eq(user)))).get_result(conn)?;
        if !user_exists {
            eprintln!("ERROR: No user with this username exists\n");
            continue;
        }

        let password = admin::prompt_user_input("New password", &mut password_buffer);
        if password == "done" {
            break;
        }
        if let Err(err) = api::verify_matches_regex(password, RegexType::Password) {
            eprintln!("ERROR: {err}\n");
            continue;
        }

        let salt = SaltString::generate(&mut OsRng);
        let hash = password::hash_password(password, &salt)?;
        diesel::update(user::table)
            .filter(user::name.eq(user))
            .set((user::password_hash.eq(&hash), user::password_salt.eq(salt.as_str())))
            .execute(conn)?;
        println!("Password reset successful.\n");
    }
    Ok(())
}

const PRINT_INTERVAL: u64 = 1000;

fn get_post_id(path: &Path) -> Option<i32> {
    let path_str = path.file_name()?.to_string_lossy();
    let (post_id, _tail) = path_str.split_once('_')?;
    post_id.parse().ok()
}
