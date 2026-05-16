use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    // This makes project rebuild if migrations folder has changed.
    // It's important because we embed migrations into application binary,
    // so we want to recompile if migrations have changed.
    println!("cargo::rerun-if-changed=migrations/");

    // Since we embed the default config, also build if it changes
    println!("cargo::rerun-if-changed=config.toml.dist");

    // Also want to recompile if templates change
    println!("cargo::rerun-if-changed=templates/");

    // Record build timestamp
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let destination = Path::new(&out_dir).join("build_timestamp.rs");
    std::fs::write(&destination, format!("pub const BUILD_TIMESTAMP: i64 = {secs};")).unwrap();
}
