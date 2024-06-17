use once_cell::sync::Lazy;
use std::path::PathBuf;
use toml::Table;

pub static CONFIG: Lazy<Table> = Lazy::new(|| {
    std::fs::read_to_string(get_config_path())
        .unwrap_or_else(|err| panic!("{err}"))
        .parse()
        .unwrap_or_else(|err| panic!("{err}"))
});

fn get_config_path() -> PathBuf {
    // Use config.toml.dist if in development environment, config.toml if in production
    match std::env::var("CARGO_MANIFEST_DIR") {
        Ok(var) => {
            let mut project_path = PathBuf::from(var);
            project_path.push("config.toml.dist");
            project_path
        }
        Err(_) => {
            let exe_path = std::env::current_exe().unwrap_or_else(|err| panic!("{err}"));
            let mut parent_path = exe_path
                .parent()
                .unwrap_or_else(|| panic!("Exe path has no parent"))
                .to_owned();
            parent_path.push("config.toml");
            parent_path
        }
    }
}
