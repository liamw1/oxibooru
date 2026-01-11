use crate::admin::{AdminTask, LoopState};
use crate::app;
use rustyline::completion::{Completer, Pair};
use rustyline::error::ReadlineError;
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::history::DefaultHistory;
use rustyline::validate::Validator;
use rustyline::{Context, Editor, Helper};
use strum::{EnumMessage, IntoEnumIterator};

pub struct TaskCompleter;

impl Completer for TaskCompleter {
    type Candidate = Pair;

    fn complete(&self, line: &str, pos: usize, _ctx: &Context<'_>) -> rustyline::Result<(usize, Vec<Pair>)> {
        // Find the start of the current word
        let start = line[..pos].rfind(' ').map_or(0, |i| i + 1);
        let prefix = &line[start..pos];

        let matches: Vec<Pair> = AdminTask::iter()
            .map(AdminTask::into)
            .filter(|task: &&str| task.starts_with(prefix))
            .map(|task| Pair {
                display: task.to_owned(),
                replacement: task.to_owned(),
            })
            .collect();

        Ok((start, matches))
    }
}

impl Hinter for TaskCompleter {
    type Hint = String;
}
impl Highlighter for TaskCompleter {}
impl Validator for TaskCompleter {}
impl Helper for TaskCompleter {}

/// Prompts the user for input with message `prompt` and reads resulting input.
pub fn read(prompt: &str, editor: &mut Editor<TaskCompleter, DefaultHistory>) -> Result<String, LoopState> {
    loop {
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

                return match LoopState::try_from(trimmed) {
                    Ok(state) => Err(state),
                    Err(()) => Ok(trimmed.to_owned()),
                };
            }
            Err(ReadlineError::Interrupted) => {
                println!("^C");
                return Err(LoopState::Exit);
            }
            Err(err) => {
                eprintln!("Error: {err}");
            }
        }
    }
}

/// Prints some helpful information about the CLI to the console.
fn print_info() {
    println!("Oxibooru admin CLI - running on {} threads\n", app::num_rayon_threads());
    println!("Commands:");
    println!("  {:12} {}", "help", "Show this help message");
    println!("  {:12} {}", "clear", "Clear the screen");
    println!("  {:12} {}", "exit", "Exit the CLI");
    println!();
    println!("Tasks:");
    for task in AdminTask::iter() {
        let name: &str = task.into();
        println!("  {:35} {}", name, task.get_message().unwrap_or_default());
    }
    println!();
}
