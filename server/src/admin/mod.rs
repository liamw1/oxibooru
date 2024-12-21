mod post;

use crate::db;
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, PooledConnection};
use std::str::FromStr;
use strum::{EnumIter, EnumString, IntoEnumIterator, IntoStaticStr};

pub fn enabled() -> bool {
    std::env::args().any(|arg| arg == "--admin")
}

pub fn command_line_mode() {
    print_info();

    let mut input = String::new();
    let stdin = std::io::stdin();
    loop {
        input.clear();
        if let Err(err) = stdin.read_line(&mut input) {
            eprintln!("{err}");
        }
        if input.trim() == "exit" {
            break;
        }

        let task = match AdminTask::from_str(input.trim()) {
            Ok(task) => task,
            Err(_) => {
                let possible_arguments: Vec<&'static str> = AdminTask::iter().map(AdminTask::into).collect();
                eprintln!("Command line arguments should be one of {possible_arguments:?}\n");
                continue;
            }
        };
        match run_task(task) {
            Ok(()) => {
                println!("Task finished.\n");
                print_info();
            }
            Err(err) => eprintln!("{err}"),
        }
    }
}

#[derive(Clone, Copy, EnumString, EnumIter, IntoStaticStr)]
#[strum(serialize_all = "snake_case")]
enum AdminTask {
    RenamePostContent,
    RecomputePostSignatures,
    RecomputePostSignatureIndexes,
    RecomputePostChecksums,
    RegenerateThumbnail,
}

fn print_info() {
    println!("Running Oxibooru in admin mode. Enter \"exit\" when finished.");
    let possible_arguments: Vec<&'static str> = AdminTask::iter().map(AdminTask::into).collect();
    println!("Available commands: {possible_arguments:?}");
    println!();
}

fn get_connection() -> Result<PooledConnection<ConnectionManager<PgConnection>>, String> {
    db::get_connection().map_err(|err| format!("Could not connect to the database: {err}"))
}

fn run_task(task: AdminTask) -> Result<(), String> {
    println!("Starting task...");
    match task {
        AdminTask::RenamePostContent => post::rename_post_content().map_err(|err| format!("{err}")),
        AdminTask::RecomputePostSignatures => {
            let mut conn = get_connection()?;
            post::recompute_signatures(&mut conn).map_err(|err| format!("{err}"))
        }
        AdminTask::RecomputePostSignatureIndexes => {
            let mut conn = get_connection()?;
            post::recompute_indexes(&mut conn).map_err(|err| format!("{err}"))
        }
        AdminTask::RecomputePostChecksums => {
            let mut conn = get_connection()?;
            post::recompute_checksums(&mut conn).map_err(|err| format!("{err}"))
        }
        AdminTask::RegenerateThumbnail => {
            let mut conn = get_connection()?;
            post::regenerate_thumbnail(&mut conn).map_err(|err| format!("{err}"))
        }
    }
}
