use crate::config;
use std::path::PathBuf;

pub fn purge_temporary_uploads() -> std::io::Result<()> {
    let data_directory = config::read_required_string("data_dir");
    let temp_path = PathBuf::from(format!("{data_directory}/temporary-uploads"));
    for entry in std::fs::read_dir(temp_path)? {
        let path = entry?.path();
        std::fs::remove_file(path)?;
    }
    Ok(())
}
