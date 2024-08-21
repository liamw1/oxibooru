use crate::api::ApiResult;
use crate::auth::content;
use crate::filesystem;
use crate::image::{read, signature};
use crate::model::enums::MimeType;
use crate::model::post::{NewPostSignature, PostSignature};
use crate::schema::{post, post_signature};
use crate::util::Timer;
use diesel::prelude::*;
use image::DynamicImage;
use std::path::Path;

pub fn rename_post_content() -> std::io::Result<()> {
    let _time = Timer::new("rename_post_content");

    for (entry_index, entry) in std::fs::read_dir(filesystem::generated_thumbnails_directory())?.enumerate() {
        let path = entry?.path();
        if let Some(post_id) = get_post_id(&path) {
            let new_path = content::post_thumbnail_path(post_id);
            if path != new_path {
                std::fs::rename(path, new_path)?;
            }
        } else {
            eprintln!("Could not find post_id of {path:?}");
        }

        print_progress_message(entry_index, "Thumbnails renamed");
    }

    for (entry_index, entry) in std::fs::read_dir(filesystem::posts_directory())?.enumerate() {
        let path = entry?.path();
        let post_id = get_post_id(&path);
        let content_type = MimeType::from_path(&path);
        if let (Some(id), Some(mime_type)) = (post_id, content_type) {
            let new_path = content::post_content_path(id, mime_type);
            if path != new_path {
                std::fs::rename(path, new_path)?;
            }
        } else {
            eprintln!("Could not find post_id or mime_type of {path:?}");
        }

        print_progress_message(entry_index, "Posts renamed");
    }

    Ok(())
}

pub fn recompute_indexes(conn: &mut PgConnection) -> QueryResult<()> {
    let _time = Timer::new("recompute_indexes");

    let post_signatures: Vec<PostSignature> = post_signature::table.select(PostSignature::as_select()).load(conn)?;
    let indexes: Vec<_> = post_signatures
        .iter()
        .map(|sig| signature::generate_indexes(&sig.signature))
        .collect();
    let new_post_signatures: Vec<_> = post_signatures
        .iter()
        .zip(indexes.iter())
        .map(|(sig, words)| NewPostSignature {
            post_id: sig.post_id,
            signature: &sig.signature,
            words,
        })
        .collect();

    diesel::delete(post_signature::table).execute(conn)?;
    diesel::insert_into(post_signature::table)
        .values(new_post_signatures)
        .execute(conn)?;

    Ok(())
}

pub fn recompute_signatures(conn: &mut PgConnection) -> QueryResult<()> {
    let _time = Timer::new("recompute_signatures");

    diesel::delete(post_signature::table).execute(conn)?;

    let posts: Vec<(i32, MimeType)> = post::table.select((post::id, post::mime_type)).load(conn)?;
    for (post_index, (post_id, mime_type)) in posts.into_iter().enumerate() {
        let image_path = content::post_content_path(post_id, mime_type);
        match decode_image(&image_path) {
            Ok((image, _)) => {
                let image_signature = signature::compute_signature(&image);
                let signature_indexes = signature::generate_indexes(&image_signature);
                let new_post_signature = NewPostSignature {
                    post_id,
                    signature: &image_signature,
                    words: &signature_indexes,
                };
                diesel::insert_into(post_signature::table)
                    .values(new_post_signature)
                    .execute(conn)?;
            }
            Err(err) => eprintln!("Unable to compute signature for post {post_id} for reason: {err}"),
        }

        print_progress_message(post_index, "Signatures computed");
    }

    Ok(())
}

pub fn recompute_checksums(conn: &mut PgConnection) -> QueryResult<()> {
    let _time = Timer::new("recompute_checksums");

    let posts: Vec<(i32, MimeType)> = post::table.select((post::id, post::mime_type)).load(conn)?;
    for (post_index, (post_id, mime_type)) in posts.into_iter().enumerate() {
        let image_path = content::post_content_path(post_id, mime_type);
        match decode_image(&image_path) {
            Ok((image, file_size)) => {
                let checksum = content::image_checksum(&image, file_size);
                let duplicate: Option<i32> = post::table
                    .select(post::id)
                    .filter(post::checksum.eq(&checksum))
                    .first(conn)
                    .optional()?;
                if duplicate.is_some() && duplicate != Some(post_id) {
                    let dup_id = duplicate.unwrap();
                    eprintln!("Potential duplicate post {dup_id} for post {post_id}");
                } else {
                    diesel::update(post::table.find(post_id))
                        .set(post::checksum.eq(checksum))
                        .execute(conn)?;
                }
            }
            Err(err) => eprintln!("Unable to compute checksum for post {post_id} for reason: {err}"),
        }

        print_progress_message(post_index, "Checksums computed");
    }

    Ok(())
}

const PRINT_INTERVAL: usize = 1000;

fn get_post_id(path: &Path) -> Option<i32> {
    let path_str = path.file_name()?.to_string_lossy();
    let (post_id, _tail) = path_str.split_once('_')?;
    post_id.parse().ok()
}

fn print_progress_message(index: usize, msg: &str) {
    if index > 0 && index % PRINT_INTERVAL == 0 {
        println!("{msg}: {index}");
    }
}

fn decode_image(path: &Path) -> ApiResult<(DynamicImage, u64)> {
    let image_metadata = std::fs::metadata(path)?;
    let image_reader = read::new_image_reader(path)?;

    let decoded_image = image_reader.decode()?;
    Ok((decoded_image, image_metadata.len()))
}
