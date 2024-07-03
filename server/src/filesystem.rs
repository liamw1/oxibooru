use crate::config;
use std::path::PathBuf;

pub fn posts_directory() -> PathBuf {
    format!("{}/posts", config::get().data_dir).into()
}

pub fn generated_thumbnails_directory() -> PathBuf {
    format!("{}/generated-thumbnails", config::get().data_dir).into()
}

pub fn temporary_upload_directory() -> PathBuf {
    format!("{}/temporary-uploads", config::get().data_dir).into()
}

pub fn temporary_upload_filepath(filename: &str) -> PathBuf {
    format!("{}/temporary-uploads/{}", config::get().data_dir, filename).into()
}

pub fn purge_temporary_uploads() -> std::io::Result<()> {
    let temp_path = temporary_upload_directory();
    if !temp_path.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(temporary_upload_directory())? {
        let path = entry?.path();
        std::fs::remove_file(path)?;
    }
    Ok(())
}
