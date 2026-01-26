use crate::admin::input::{CancelType, PostEditor, TaskEditor, UserEditor};
use crate::api::error::ApiError;
use crate::app::{self, AppState};
use crate::auth::Client;
use crate::model::enums::UserRank;
use diesel::r2d2::PoolError;
use rayon::ThreadPoolBuilder;
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

pub type AdminResult<T> = Result<T, AdminError>;

#[derive(Debug, Error)]
#[error(transparent)]
pub enum AdminError {
    Cancel(#[from] CancelType),
    #[error("{0}")]
    Error(String),
}

impl From<&str> for AdminError {
    fn from(value: &str) -> Self {
        Self::Error(value.to_owned())
    }
}

impl From<String> for AdminError {
    fn from(value: String) -> Self {
        Self::Error(value)
    }
}

impl From<PoolError> for AdminError {
    fn from(value: PoolError) -> Self {
        Self::Error(value.to_string())
    }
}

impl From<diesel::result::Error> for AdminError {
    fn from(value: diesel::result::Error) -> Self {
        Self::Error(value.to_string())
    }
}

impl From<std::io::Error> for AdminError {
    fn from(value: std::io::Error) -> Self {
        Self::Error(value.to_string())
    }
}

impl From<walkdir::Error> for AdminError {
    fn from(value: walkdir::Error) -> Self {
        Self::Error(value.to_string())
    }
}

impl From<ApiError> for AdminError {
    fn from(value: ApiError) -> Self {
        Self::Error(value.to_string())
    }
}

#[derive(Clone, Copy, EnumIter, EnumString, EnumMessage, IntoStaticStr)]
#[strum(serialize_all = "snake_case")]
pub enum AdminTask {
    #[strum(message = "Checks integrity of post files")]
    CheckIntegrity,
    #[strum(message = "Recompute post checksums")]
    RecomputeChecksums,
    #[strum(message = "Rebuild post signatures")]
    RecomputeSignatures,
    #[strum(message = "Rebuild reverse search index")]
    RecomputeIndex,
    #[strum(message = "Regenerate post thumbnails")]
    RegenerateThumbnails,
    #[strum(message = "Reset user passwords")]
    ResetPasswords,
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
    println!("Enter \"help\" for a list of commands, \"done\" to escape current task and \"exit\" when finished.\n");

    // Set up signal handlers to cancel long-running tasks
    install_signal_handlers();

    ThreadPoolBuilder::new()
        .num_threads(app::num_rayon_threads())
        .build_global()
        .expect("Must be able to configure to global rayon thread pool");

    let mut post_editor = input::create_editor();
    let mut task_editor = input::create_editor();
    let mut user_editor = input::create_editor();
    input::user_input_loop(state, &mut task_editor, |state: &AppState, editor: &mut TaskEditor| {
        let user_input = input::read("Please select a task: ", editor)?;

        let task = AdminTask::from_str(&user_input).map_err(|_| {
            let possible_arguments: Vec<&str> = AdminTask::iter().map(AdminTask::into).collect();
            format!("Command line arguments must be one of {possible_arguments:?}")
        })?;
        run_task(state, task, &mut post_editor, &mut user_editor);

        if CANCELLED.load(Ordering::SeqCst) {
            error!("Task aborted.\n");
        }
        Ok(())
    });
}

pub fn mock_editor() -> PostEditor {
    input::create_mock_editor()
}

const PRINT_INTERVAL: Option<u64> = Some(1000);
static CANCELLED: LazyLock<Arc<AtomicBool>> = LazyLock::new(|| Arc::new(AtomicBool::new(false)));

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

fn is_cancelled() -> Result<(), CancelType> {
    if CANCELLED.load(Ordering::SeqCst) {
        Err(CancelType::Stop)
    } else {
        Ok(())
    }
}

fn client() -> Client {
    Client {
        id: None,
        rank: UserRank::Administrator,
    }
}

/// Runs a single task. This function is designed to only establish a connection to the database
/// if necessary. That way users can run tasks that don't require database connection without
/// spinning up the database.
fn run_task(state: &AppState, task: AdminTask, post_editor: &mut PostEditor, user_editor: &mut UserEditor) {
    CANCELLED.store(false, Ordering::SeqCst);
    match task {
        AdminTask::CheckIntegrity => post::check_integrity(state, post_editor),
        AdminTask::RecomputeChecksums => post::recompute_checksums(state, post_editor),
        AdminTask::RecomputeSignatures => post::recompute_signatures(state, post_editor),
        AdminTask::RecomputeIndex => post::recompute_indexes(state, post_editor),
        AdminTask::RegenerateThumbnails => post::regenerate_thumbnails(state, post_editor),
        AdminTask::ResetPasswords => user::reset_password(state, user_editor),
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
