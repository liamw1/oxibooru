use crate::admin::input::TaskCompleter;
use crate::app::{self, AppState};
use diesel::r2d2::PoolError;
use rayon::ThreadPoolBuilder;
use rustyline::history::DefaultHistory;
use rustyline::{CompletionType, Config, Editor};
use signal_hook::consts::{SIGINT, SIGTERM};
use std::path::Path;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, LazyLock};
use strum::{EnumIter, EnumMessage, EnumString, IntoEnumIterator, IntoStaticStr};
use thiserror::Error;
use tracing::{error, info};

pub mod database;
mod input;
pub mod post;
mod user;

pub type DatabaseResult<T> = Result<T, DatabaseError>;

#[derive(Debug, Error)]
#[error("Task was cancelled")]
pub struct CancellationError;

#[derive(Debug, Error)]
#[error(transparent)]
pub enum DatabaseError {
    Cancelled(#[from] CancellationError),
    Connection(#[from] PoolError),
    Query(#[from] diesel::result::Error),
    Io(#[from] std::io::Error),
    WalkDir(#[from] walkdir::Error),
}

#[derive(Clone, Copy, EnumIter, EnumString, EnumMessage, IntoStaticStr)]
#[strum(serialize_all = "snake_case")]
pub enum AdminTask {
    #[strum(message = "Checks integrity of post files")]
    CheckPostIntegrity,
    #[strum(message = "Recompute post checksums")]
    RecomputePostChecksums,
    #[strum(message = "Rebuild post signatures")]
    RecomputePostSignatures,
    #[strum(message = "Rebuild search index")]
    RecomputePostSignatureIndexes,
    #[strum(message = "Regenerate all post thumbnails")]
    RegenerateThumbnails,
    #[strum(message = "Regenerate specific post thumbnails")]
    RegenerateThumbnail,
    #[strum(message = "Reset individual user passwords")]
    ResetPassword,
    #[strum(message = "Rebuild data directory")]
    ResetFilenames,
    #[strum(message = "Rebuild table statistics")]
    ResetStatistics,
    #[strum(message = "Cache thumbnail sizes")]
    ResetThumbnailSizes,
}

/// Checks if server was started in admin mode.
pub fn enabled() -> bool {
    std::env::args().any(|arg| arg == "--admin")
}

/// Starts server CLI.
pub fn command_line_mode(state: &AppState) {
    println!("Running Oxibooru admin command line interface on {} threads.", app::num_rayon_threads());
    println!("Enter \"help\" for a list of commands and \"exit\" when finished.\n");

    // Set up signal handlers to cancel long-running tasks
    install_signal_handlers();

    ThreadPoolBuilder::new()
        .num_threads(app::num_rayon_threads())
        .build_global()
        .expect("Must be able to configure to global rayon thread pool");

    user_input_loop(state, |state: &AppState, editor: &mut Editor<TaskCompleter, DefaultHistory>| {
        let user_input = match input::read("Please select a task: ", editor) {
            Ok(input) => input,
            Err(state) => return Ok(state),
        };

        let task = AdminTask::from_str(&user_input).map_err(|_| {
            let possible_arguments: Vec<&str> = AdminTask::iter().map(AdminTask::into).collect();
            format!("Command line arguments must be one of {possible_arguments:?}")
        })?;
        run_task(state, task).map_err(|err| err.to_string())?;

        println!("Task finished.\n");
        Ok(LoopState::Continue)
    });
}

const PRINT_INTERVAL: Option<u64> = Some(1000);
static CANCELLED: LazyLock<Arc<AtomicBool>> = LazyLock::new(|| Arc::new(AtomicBool::new(false)));

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

fn is_cancelled() -> Result<(), CancellationError> {
    if CANCELLED.load(Ordering::SeqCst) {
        Err(CancellationError)
    } else {
        Ok(())
    }
}

/// Repeatedly performs some `function` that prompts for user input until it returns
/// either [`LoopState::Stop`] or [`LoopState::Exit`], the latter of which terminates
/// the program immediately.
fn user_input_loop<F>(state: &AppState, mut function: F)
where
    F: FnMut(&AppState, &mut Editor<TaskCompleter, DefaultHistory>) -> Result<LoopState, String>,
{
    let editor_config = Config::builder().completion_type(CompletionType::List).build();
    let mut editor = Editor::with_config(editor_config).expect("Must be able to construct editor");
    editor.set_helper(Some(TaskCompleter));
    loop {
        match function(state, &mut editor) {
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
    CANCELLED.store(false, Ordering::SeqCst);
    info!("Starting task...");

    dbg!(state.config.path(crate::filesystem::Directory::GeneratedThumbnails));
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
        AdminTask::ResetFilenames => database::reset_filenames(state),
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

fn install_signal_handlers() {
    const MESSAGE: &str = "Must be able to register signal handler";
    signal_hook::flag::register(SIGINT, CANCELLED.clone()).expect(MESSAGE);
    signal_hook::flag::register(SIGTERM, CANCELLED.clone()).expect(MESSAGE);
}
