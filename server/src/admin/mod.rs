use diesel::prelude::*;
use diesel::r2d2::PoolError;
use std::io::Write;
use std::path::Path;
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use strum::{EnumIter, EnumString, IntoEnumIterator, IntoStaticStr};
use thiserror::Error;
use tracing::{error, info};

pub mod database;
mod post;
mod user;

pub fn enabled() -> bool {
    std::env::args().any(|arg| arg == "--admin")
}

pub fn command_line_mode(conn: &mut PgConnection) {
    print_info();

    user_input_loop(conn, |conn: &mut PgConnection, buffer: &mut String| {
        let user_input = prompt_user_input("Please select a task", buffer);
        if let Ok(state) = LoopState::try_from(user_input) {
            return Ok(state);
        }

        let task = AdminTask::from_str(user_input).map_err(|_| {
            let possible_arguments: Vec<&'static str> = AdminTask::iter().map(AdminTask::into).collect();
            format!("Command line arguments should be one of {possible_arguments:?}")
        })?;
        run_task(conn, task).map_err(|err| err.to_string())?;

        println!("Task finished.\n");
        Ok(LoopState::Continue)
    });
}

const PRINT_INTERVAL: u64 = 1000;

#[derive(Debug, Error)]
#[error(transparent)]
pub enum DatabaseError {
    Connection(#[from] PoolError),
    Query(#[from] diesel::result::Error),
}

pub type DatabaseResult<T> = Result<T, DatabaseError>;

#[derive(Clone, Copy, EnumString, EnumIter, IntoStaticStr)]
#[strum(serialize_all = "snake_case")]
pub enum AdminTask {
    RecomputePostChecksums,
    RecomputePostSignatures,
    RecomputePostSignatureIndexes,
    RegenerateThumbnails,
    RegenerateThumbnail,
    ResetPassword,
    ResetFilenames,
    ResetStatistics,
    ResetThumbnailSizes,
}

enum LoopState {
    Continue,
    Stop,
    Exit,
}

impl TryFrom<&str> for LoopState {
    type Error = ();
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "done" => Ok(LoopState::Stop),
            "exit" => Ok(LoopState::Exit),
            _ => Err(()),
        }
    }
}

struct ProgressReporter {
    message: &'static str,
    print_interval: u64,
    count: AtomicU64,
}

impl ProgressReporter {
    fn new(message: &'static str, print_interval: u64) -> Self {
        Self {
            message,
            print_interval,
            count: AtomicU64::new(0),
        }
    }

    fn increment(&self) {
        let count = self.count.fetch_add(1, Ordering::SeqCst) + 1;
        if count % self.print_interval == 0 {
            self.report();
        }
    }

    fn report(&self) {
        info!("{}: {}", self.message, self.count.load(Ordering::SeqCst));
    }
}

fn print_info() {
    let possible_arguments: Vec<&'static str> = AdminTask::iter().map(AdminTask::into).collect();
    println!(
        "Running Oxibooru admin command line interface. Enter \"help\" for a list of commands and \"exit\" when finished."
    );
    println!("Available commands: {possible_arguments:?}\n");
}

fn prompt_user_input<'a>(prompt: &str, buffer: &'a mut String) -> &'a str {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    loop {
        print!("{prompt}: ");
        if let Err(err) = stdout.flush() {
            error!("{err}\n");
            continue;
        }

        buffer.clear();
        if let Err(err) = stdin.read_line(buffer) {
            error!("{err}\n");
            continue;
        }

        match buffer.trim() {
            "help" => {
                println!();
                print_info();
                continue;
            }
            _ => return buffer.trim(),
        }
    }
}

fn user_input_loop<F>(conn: &mut PgConnection, mut function: F)
where
    F: FnMut(&mut PgConnection, &mut String) -> Result<LoopState, String>,
{
    let mut buffer = String::new();
    loop {
        match function(conn, &mut buffer) {
            Ok(LoopState::Continue) => continue,
            Ok(LoopState::Stop) => break,
            Ok(LoopState::Exit) => std::process::exit(0),
            Err(err) => {
                error!("{err}\n");
                continue;
            }
        }
    }
}

/// Runs a single task. This function is designed to only establish a connection to the database
/// if necessary. That way users can run tasks that don't require database connection without
/// spinning up the database.
fn run_task(conn: &mut PgConnection, task: AdminTask) -> Result<(), String> {
    info!("Starting task...");

    match task {
        AdminTask::RecomputePostChecksums => post::recompute_checksums(conn).map_err(|err| err.to_string()),
        AdminTask::RecomputePostSignatures => post::recompute_signatures(conn).map_err(|err| err.to_string()),
        AdminTask::RecomputePostSignatureIndexes => post::recompute_indexes(conn).map_err(|err| err.to_string()),
        AdminTask::RegenerateThumbnails => post::regenerate_thumbnails(conn).map_err(|err| err.to_string()),
        AdminTask::RegenerateThumbnail => {
            post::regenerate_thumbnail(conn);
            Ok(())
        }
        AdminTask::ResetPassword => {
            user::reset_password(conn);
            Ok(())
        }
        AdminTask::ResetFilenames => database::reset_filenames().map_err(|err| err.to_string()),
        AdminTask::ResetStatistics => database::reset_statistics().map_err(|err| err.to_string()),
        AdminTask::ResetThumbnailSizes => database::reset_thumbnail_sizes(conn).map_err(|err| err.to_string()),
    }
}

fn get_post_id(path: &Path) -> Option<i64> {
    let path_str = path.file_name()?.to_string_lossy();
    let (post_id, _tail) = path_str.split_once('_')?;
    post_id.parse().ok()
}
