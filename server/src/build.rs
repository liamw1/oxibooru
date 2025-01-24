/// This makes project rebuild if migrations folder has changed.
/// It's important because we embed migrations into application binary,
/// so we want to recompile if migrations have changed.
fn main() {
    println!("cargo::rerun-if-changed=migrations/");
}
