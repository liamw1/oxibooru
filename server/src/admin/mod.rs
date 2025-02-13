pub mod database;
mod post;
mod user;

use crate::db;
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, PooledConnection};
use std::io::Write;
use std::path::Path;
use std::str::FromStr;
use strum::{EnumIter, EnumString, IntoEnumIterator, IntoStaticStr};

pub fn enabled() -> bool {
    std::env::args().any(|arg| arg == "--admin")
}

pub fn command_line_mode() {
    print_info();

    let mut buffer = String::new();
    loop {
        let user_input = prompt_user_input("Please select a task", &mut buffer);
        let task = match AdminTask::from_str(user_input) {
            Ok(task) => task,
            Err(_) => {
                let possible_arguments: Vec<&'static str> = AdminTask::iter().map(AdminTask::into).collect();
                eprintln!("ERROR: Command line arguments should be one of {possible_arguments:?}\n");
                continue;
            }
        };
        match run_task(task) {
            Ok(()) => println!("Task finished.\n"),
            Err(err) => eprintln!("ERROR: {err}\n"),
        }
    }
}

const PRINT_INTERVAL: u64 = 1000;

#[derive(Clone, Copy, EnumString, EnumIter, IntoStaticStr)]
#[strum(serialize_all = "snake_case")]
enum AdminTask {
    RecomputePostChecksums,
    RecomputePostSignatures,
    RecomputePostSignatureIndexes,
    RegenerateThumbnail,
    ResetPassword,
    ResetFilenames,
    ResetStatistics,
    ResetThumbnailSizes,
}

struct ProgressReporter {
    message: &'static str,
    print_interval: u64,
    count: u64,
}

impl ProgressReporter {
    fn new(message: &'static str, print_interval: u64) -> Self {
        Self {
            message,
            print_interval,
            count: 0,
        }
    }

    fn increment(&mut self) {
        self.count += 1;
        if self.count > 0 && self.count % self.print_interval == 0 {
            self.report();
        }
    }

    fn report(&self) {
        println!("{}: {}", self.message, self.count);
    }
}

fn get_connection() -> Result<PooledConnection<ConnectionManager<PgConnection>>, String> {
    db::get_connection().map_err(|err| format!("Could not connect to the database: {err}"))
}

fn print_info() {
    let possible_arguments: Vec<&'static str> = AdminTask::iter().map(AdminTask::into).collect();
    println!("Running Oxibooru admin command line interface. Enter \"help\" for a list of commands and \"exit\" when finished.");
    println!("Available commands: {possible_arguments:?}\n");
}

fn prompt_user_input<'a>(prompt: &str, buffer: &'a mut String) -> &'a str {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    loop {
        print!("{prompt}: ");
        if let Err(err) = stdout.flush() {
            eprintln!("ERROR: {err}\n");
            continue;
        }

        buffer.clear();
        if let Err(err) = stdin.read_line(buffer) {
            eprintln!("ERROR: {err}\n");
            continue;
        }

        match buffer.trim() {
            "exit" => std::process::exit(0),
            "help" => {
                println!();
                print_info();
                continue;
            }
            _ => return buffer.trim(),
        }
    }
}

/// Runs a single task. This function is designed to only establish a connection to the database
/// if necessary. That way users can run tasks that don't require database connection without
/// spinning up the database.
fn run_task(task: AdminTask) -> Result<(), String> {
    println!("Starting task...");
    match task {
        AdminTask::RecomputePostChecksums => {
            let mut conn = get_connection()?;
            post::recompute_checksums(&mut conn).map_err(|err| format!("{err}"))
        }
        AdminTask::RecomputePostSignatures => {
            let mut conn = get_connection()?;
            post::recompute_signatures(&mut conn).map_err(|err| format!("{err}"))
        }
        AdminTask::RecomputePostSignatureIndexes => {
            let mut conn = get_connection()?;
            post::recompute_indexes(&mut conn).map_err(|err| format!("{err}"))
        }
        AdminTask::RegenerateThumbnail => {
            let mut conn = get_connection()?;
            post::regenerate_thumbnail(&mut conn).map_err(|err| format!("{err}"))
        }
        AdminTask::ResetPassword => {
            let mut conn = get_connection()?;
            user::reset_password(&mut conn).map_err(|err| format!("{err}"))
        }
        AdminTask::ResetFilenames => database::reset_filenames().map_err(|err| format!("{err}")),
        AdminTask::ResetStatistics => {
            let mut conn = get_connection()?;
            database::reset_statistics(&mut conn).map_err(|err| format!("{err}"))
        }
        AdminTask::ResetThumbnailSizes => {
            let mut conn = get_connection()?;
            database::reset_thumbnail_sizes(&mut conn).map_err(|err| format!("{err}"))
        }
    }
}

fn get_post_id(path: &Path) -> Option<i64> {
    let path_str = path.file_name()?.to_string_lossy();
    let (post_id, _tail) = path_str.split_once('_')?;
    post_id.parse().ok()
}
