fn main() {
    /// This makes project rebuild if migrations folder has changed.
    /// It's important because we embed migrations into application binary,
    /// so we want to recompile if migrations have changed.
    println!("cargo::rerun-if-changed=migrations/");

    /// Since we embed the default config, also build if it changes
    println!("cargo::rerun-if-changed=config.toml.dist");
}
