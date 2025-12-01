use crate::app::{self, AppState};
use diesel::r2d2::PoolError;
use rayon::ThreadPoolBuilder;
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

pub type DatabaseResult<T> = Result<T, DatabaseError>;

#[derive(Debug, Error)]
#[error(transparent)]
pub enum DatabaseError {
    Connection(#[from] PoolError),
    Query(#[from] diesel::result::Error),
    Io(#[from] std::io::Error),
}

#[derive(Clone, Copy, EnumString, EnumIter, IntoStaticStr)]
#[strum(serialize_all = "snake_case")]
pub enum AdminTask {
    CheckPostIntegrity,
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

/// Checks if server was started in admin mode.
pub fn enabled() -> bool {
    std::env::args().any(|arg| arg == "--admin")
}

/// Starts server CLI.
pub fn command_line_mode(state: &AppState) {
    print_info();

    ThreadPoolBuilder::new()
        .num_threads(app::num_rayon_threads())
        .build_global()
        .expect("Must be able to configure to global rayon thread pool");

    user_input_loop(state, |state: &AppState, buffer: &mut String| {
        let user_input = prompt_user_input("Please select a task", buffer);
        if let Ok(state) = LoopState::try_from(user_input) {
            return Ok(state);
        }

        let task = AdminTask::from_str(user_input).map_err(|_| {
            let possible_arguments: Vec<&'static str> = AdminTask::iter().map(AdminTask::into).collect();
            format!("Command line arguments must be one of {possible_arguments:?}")
        })?;
        run_task(state, task).map_err(|err| err.to_string())?;

        println!("Task finished.\n");
        Ok(LoopState::Continue)
    });
}

const PRINT_INTERVAL: Option<u64> = Some(1000);

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

/// An atomic counter that prints message with the current count at regular intervals.
struct ProgressReporter {
    message: &'static str,
    print_interval: Option<u64>,
    count: AtomicU64,
}

impl ProgressReporter {
    /// Creates a new [`ProgressReporter`] that will print "`message`: {count}"
    /// to the info logs every `print_interval` increments.
    fn new(message: &'static str, print_interval: Option<u64>) -> Self {
        Self {
            message,
            print_interval,
            count: AtomicU64::new(0),
        }
    }

    /// Atomically increments the count.
    fn increment(&self) {
        let count = self.count.fetch_add(1, Ordering::SeqCst) + 1;
        if let Some(print_interval) = self.print_interval
            && count.is_multiple_of(print_interval)
        {
            self.report();
        }
    }

    /// Immediately prints "{message}: {count}" to the info logs.
    fn report(&self) {
        info!("{}: {}", self.message, self.count.load(Ordering::SeqCst));
    }
}

/// Prints some helpful information about the CLI to the console.
fn print_info() {
    let possible_arguments: Vec<&'static str> = AdminTask::iter().map(AdminTask::into).collect();
    println!(
        "Running Oxibooru admin command line interface on {} threads.
        Enter \"help\" for a list of commands and \"exit\" when finished.",
        app::num_rayon_threads()
    );
    println!("Available commands: {possible_arguments:?}\n");
}

/// Prompts the user for input with message `prompt`.
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
            }
            _ => return buffer.trim(),
        }
    }
}

/// Repeatedly performs some `function` that prompts for user input until it returns
/// either [`LoopState::Stop`] or [`LoopState::Exit`], the latter of which terminates
/// the program immediately.
fn user_input_loop<F>(state: &AppState, mut function: F)
where
    F: FnMut(&AppState, &mut String) -> Result<LoopState, String>,
{
    let mut buffer = String::new();
    loop {
        match function(state, &mut buffer) {
            Ok(LoopState::Continue) => (),
            Ok(LoopState::Stop) => break,
            Ok(LoopState::Exit) => std::process::exit(0),
            Err(err) => {
                error!("{err}\n");
            }
        }
    }
}

/// Runs a single task. This function is designed to only establish a connection to the database
/// if necessary. That way users can run tasks that don't require database connection without
/// spinning up the database.
fn run_task(state: &AppState, task: AdminTask) -> DatabaseResult<()> {
    info!("Starting task...");

    match task {
        AdminTask::CheckPostIntegrity => post::check_integrity(state),
        AdminTask::RecomputePostChecksums => post::recompute_checksums(state),
        AdminTask::RecomputePostSignatures => post::recompute_signatures(state),
        AdminTask::RecomputePostSignatureIndexes => post::recompute_indexes(state),
        AdminTask::RegenerateThumbnails => post::regenerate_thumbnails(state),
        AdminTask::RegenerateThumbnail => {
            post::regenerate_thumbnail(state);
            Ok(())
        }
        AdminTask::ResetPassword => {
            user::reset_password(state);
            Ok(())
        }
        AdminTask::ResetFilenames => database::reset_filenames(state).map_err(DatabaseError::from),
        AdminTask::ResetStatistics => database::reset_statistics(state),
        AdminTask::ResetThumbnailSizes => database::reset_thumbnail_sizes(state),
    }
}

/// Extrats the post ID from a `path` to post content.
fn get_post_id(path: &Path) -> Option<i64> {
    let path_str = path.file_name()?.to_string_lossy();
    let (post_id, _tail) = path_str.split_once('_')?;
    post_id.parse().ok()
}
