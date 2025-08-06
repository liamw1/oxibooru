# Development Guide
## Installing Dependencies
To get started it's recommended to install the Rust toolchain using [`rustup`](https://www.rust-lang.org/tools/install). If you've already installed `rustup` before, you can update with
```console
rustup update
```
To aid in the editing and navigating of Rust code, install `rust-analyzer` in your development environment of choice. I highly recommend turning on your editors format-on-save feature to keep the code properly formatted.

Next, you'll want to install the build and runtime dependencies. On Debian-based systems, this looks something like
```console
sudo apt-get update && sudo apt-get install -y clang pkg-config libssl-dev libpq-dev libavcodec-dev libavformat-dev libavutil-dev libavfilter-dev libavdevice-dev
```

## Rust Basics
### Compiling
To compile the server, enter the `server/` directory and run
```console
cargo build
```
If you would just like to check if the server compiles, you can run
```console
cargo check
```
This is much faster than building, as it skips the expensive code generation step of compilation. Finally, to run, run
```console
cargo run
```
Note that by default, these commands operate on the `debug` profile, which adds extra checks and symbols useful for debugging. However, `debug` is usually 10-100x slower than `release` builds. If you would like to compile the `release` profile, you can add the `--release` flag can be added to any of these commands.

### Testing
The server-side code has a number of unit tests, most of which require a connection to the database. Make sure the PostgreSQL server is up and ready to make connections before running tests.

To run all server unit tests, run
```console
cargo test
```
Some tests take a few minutes to run in `debug`, so you may wish to add `--release` to speed things up. By default, the test runner hides any console output during tests. You can opt out of this behavior with the following flag:
```console
cargo test -- --nocapture
```
You can also run a subset of tests using
```console
cargo test test_filter
```
where only tests with `test_filter` in their name or namespace will be run. You can learn about additional test options [here](https://doc.rust-lang.org/cargo/commands/cargo-test.html).

If there are any test failures, make sure to start your investigation at the first test failure. Some test failures can cause other failures due to database consistency issues.

### Linting
After a series of changes to the code, it's good practice to run Rust's static analysis tool, `clippy`, to help catch subtle bugs and style issues. You can do this by running
```console
cargo clippy
```

## Adding a Migration
Sometimes it is necessary to make modifications to the database schema. To accomplish this, you must add a *migration* to the `oxibooru/server/migrations/` directory. These migrations consist of a `up.sql` that makes the modification and a `down.sql` that reverts the modification.

Diesel uses the `schema.rs` file to keep track of the current state of the database in order to make compile-time checks on constructed queries. However, this is not automatically updated when a migration is added. DO NOT update `schema.rs` manually. Instead, use the Diesel CLI to run the new migration and generate a new `schema.rs`.

### Using the Diesel CLI
If you haven't already, install the [Diesel CLI](https://diesel.rs/guides/getting-started.html#installing-diesel-cli). Once you've down this, navigate to the server directory and run
```console
diesel migration run --database-url='postgres://POSTGRES_USER:POSTGRES_PASSWORD@localhost:POSTGRES_PORT/POSTGRES_DB'
```
where `POSTGRES_USER`, `POSTGRES_PASSWORD`, `POSTGRES_PORT`, and `POSTGRES_DB` are the values of the environment variables defined in `.env`.

Each migration is run inside a transaction and will be reverted automatically if an error is encountered. After a migration is applied successfully, it's good practice to make sure that it is reversible. To do this, run
```console
diesel migration redo --database-url='postgres://POSTGRES_USER:POSTGRES_PASSWORD@localhost:POSTGRES_PORT/POSTGRES_DB'
```
which applies the `down.sql` and then `up.sql` again.