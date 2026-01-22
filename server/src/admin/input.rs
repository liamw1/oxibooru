use crate::admin::{AdminError, AdminResult, AdminTask};
use crate::app::AppState;
use crate::search::post::Token as PostToken;
use crate::search::user::Token as UserToken;
use rustyline::completion::{Completer, Pair};
use rustyline::error::ReadlineError;
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::history::DefaultHistory;
use rustyline::validate::Validator;
use rustyline::{CompletionType, Config, Context, Editor, Helper};
use std::marker::PhantomData;
use strum::{EnumMessage, IntoEnumIterator};
use thiserror::Error;
use tracing::error;

pub type PostEditor = Editor<EnumCompleter<PostToken>, DefaultHistory>;
pub type TaskEditor = Editor<EnumCompleter<AdminTask>, DefaultHistory>;
pub type UserEditor = Editor<EnumCompleter<UserToken>, DefaultHistory>;

#[derive(Debug, Error)]
pub enum CancelType {
    #[error("User has cancelled task")]
    Stop,
    #[error("User has exited program")]
    Exit,
}

impl TryFrom<&str> for CancelType {
    type Error = ();
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "done" => Ok(CancelType::Stop),
            "exit" => Ok(CancelType::Exit),
            _ => Err(()),
        }
    }
}

pub struct EnumCompleter<E> {
    response_count: Option<usize>,
    _phantom_data: PhantomData<E>,
}

impl<E> EnumCompleter<E> {
    pub fn new() -> Self {
        Self {
            response_count: None,
            _phantom_data: PhantomData,
        }
    }

    pub fn mocked() -> Self {
        Self {
            response_count: Some(0),
            _phantom_data: PhantomData,
        }
    }
}

impl<E: IntoEnumIterator + Into<&'static str>> Completer for EnumCompleter<E> {
    type Candidate = Pair;

    fn complete(&self, line: &str, pos: usize, _ctx: &Context<'_>) -> rustyline::Result<(usize, Vec<Pair>)> {
        // Find the start of the current word
        let start = line[..pos].rfind(' ').map_or(0, |i| i + 1);
        let prefix = &line[start..pos];

        let matches: Vec<Pair> = E::iter()
            .map(E::into)
            .filter(|task: &&str| task.starts_with(prefix))
            .map(|task| Pair {
                display: task.to_owned(),
                replacement: task.to_owned(),
            })
            .collect();

        Ok((start, matches))
    }
}

impl<E> Hinter for EnumCompleter<E> {
    type Hint = String;
}
impl<E> Highlighter for EnumCompleter<E> {}
impl<E> Validator for EnumCompleter<E> {}
impl<E: IntoEnumIterator + Into<&'static str>> Helper for EnumCompleter<E> {}

pub fn create_editor<E>() -> Editor<EnumCompleter<E>, DefaultHistory>
where
    E: IntoEnumIterator + Into<&'static str>,
{
    let editor_config = Config::builder().completion_type(CompletionType::List).build();
    let mut editor = Editor::with_config(editor_config).expect("Must be able to construct editor");
    editor.set_helper(Some(EnumCompleter::new()));
    editor
}

pub fn create_mock_editor<E>() -> Editor<EnumCompleter<E>, DefaultHistory>
where
    E: IntoEnumIterator + Into<&'static str>,
{
    let editor_config = Config::builder().completion_type(CompletionType::List).build();
    let mut editor = Editor::with_config(editor_config).expect("Must be able to construct editor");
    editor.set_helper(Some(EnumCompleter::mocked()));
    editor
}

/// Prompts the user for input with message `prompt` and reads resulting input.
pub fn read<E>(prompt: &str, editor: &mut Editor<EnumCompleter<E>, DefaultHistory>) -> Result<String, CancelType>
where
    E: IntoEnumIterator + Into<&'static str>,
{
    loop {
        if let Some(response_count) = editor.helper_mut().and_then(|helper| helper.response_count.as_mut()) {
            let result = match response_count {
                0 => Ok(String::new()),
                _ => Err(CancelType::Stop),
            };
            *response_count += 1;
            return result;
        }

        match editor.readline(prompt) {
            Ok(line) => {
                let trimmed = line.trim();
                editor.add_history_entry(trimmed).ok();
                if trimmed == "help" {
                    println!();
                    print_info();
                    continue;
                }
                if trimmed == "clear" {
                    editor.clear_screen().ok();
                    continue;
                }

                return match CancelType::try_from(trimmed) {
                    Ok(state) => Err(state),
                    Err(()) => Ok(trimmed.to_owned()),
                };
            }
            Err(ReadlineError::Interrupted) => {
                println!("^C");
                return Err(CancelType::Exit);
            }
            Err(err) => {
                eprintln!("Error: {err}");
            }
        }
    }
}

/// Repeatedly performs some `function` that prompts for user input until it returns
/// either [`LoopState::Stop`] or [`LoopState::Exit`], the latter of which terminates
/// the program immediately.
pub fn user_input_loop<F, E>(state: &AppState, editor: &mut Editor<EnumCompleter<E>, DefaultHistory>, mut function: F)
where
    F: FnMut(&AppState, &mut Editor<EnumCompleter<E>, DefaultHistory>) -> AdminResult<()>,
    E: IntoEnumIterator + Into<&'static str>,
{
    loop {
        match function(state, editor) {
            Ok(()) => (),
            Err(AdminError::Cancel(CancelType::Stop)) => break,
            Err(AdminError::Cancel(CancelType::Exit)) => std::process::exit(0),
            Err(err) => {
                error!("{err}\n");
            }
        }
    }
}

/// Prints some helpful information about the CLI to the console.
fn print_info() {
    let task_spacing = AdminTask::iter()
        .map(AdminTask::into)
        .map(|name: &str| name.len())
        .max()
        .unwrap_or(0)
        + 4;

    println!("Commands:");
    println!("  {:12} Show this help message", "help");
    println!("  {:12} Clear the screen", "clear");
    println!("  {:12} Exit the CLI", "exit");
    println!();
    println!("Tasks:");
    for task in AdminTask::iter() {
        let name: &str = task.into();
        println!("  {:task_spacing$} {}", name, task.get_message().unwrap_or_default());
    }
    println!();
    println!("Post Selection:");
    println!("  When prompted to select posts, enter a search query to filter results.");
    println!("  Leave blank to select all posts. Supports anonymous ID filters and named filters.");
    println!();
    println!("    Example: 100..500 -tag::tagme");
    println!();
    println!("  This will operate on all posts with an ID between 100 and 500 that aren't tagged with `tagme`.");
    println!();
    println!("Tip: Press Ctrl+C to gracefully cancel a running task.");
    println!();
}
