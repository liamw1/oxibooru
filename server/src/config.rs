use once_cell::sync::Lazy;
use std::path::PathBuf;
use toml::Table;

pub static CONFIG: Lazy<Table> = Lazy::new(|| std::fs::read_to_string(get_config_path()).unwrap().parse().unwrap());

pub fn read_required_string(name: &'static str) -> &'static str {
    CONFIG
        .get(name)
        .unwrap_or_else(|| panic!("Config {name} missing from config.toml"))
        .as_str()
        .unwrap_or_else(|| panic!("Config {name} is not a string"))
}

pub fn read_required_table(name: &'static str) -> &'static Table {
    CONFIG
        .get(name)
        .unwrap_or_else(|| panic!("Config {name} missing from config.toml"))
        .as_table()
        .unwrap_or_else(|| panic!("Config {name} is not a table"))
}

fn get_config_path() -> PathBuf {
    // Use config.toml.dist if in development environment, config.toml if in production
    match std::env::var("CARGO_MANIFEST_DIR") {
        Ok(var) => {
            let mut project_path = PathBuf::from(var);
            project_path.push("config.toml.dist");
            project_path
        }
        Err(_) => {
            let exe_path = std::env::current_exe().unwrap();
            let mut parent_path = exe_path.parent().expect("Exe path should have parent").to_owned();
            parent_path.push("config.toml");
            parent_path
        }
    }
}
