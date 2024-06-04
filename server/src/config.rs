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
    let filename = match std::env::var("USE_DIST_CONFIG") {
        Ok(_) => "config.toml.dist",
        Err(_) => "config.toml",
    };

    let mut path = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|err| panic!("{err}")));
    path.push(filename);
    path
}
