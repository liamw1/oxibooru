mod post;

use crate::db;
use std::str::FromStr;
use strum::{EnumIter, EnumString, IntoEnumIterator, IntoStaticStr};

pub fn run_tasks() -> usize {
    let parsed_arguments: Vec<AdminTask> = std::env::args()
        .skip(1)
        .map(|arg| AdminTask::from_str(&arg))
        .collect::<Result<_, _>>()
        .unwrap_or_else(|_| panic!("{}", error_message()));

    let mut conn = db::get_connection().unwrap();
    for &arg in parsed_arguments.iter() {
        match arg {
            AdminTask::RenamePostContent => post::rename_post_content().unwrap(),
            AdminTask::RecomputePostSignatures => post::recompute_signatures(&mut conn).unwrap(),
            AdminTask::RecomputePostSignatureIndexes => post::recompute_indexes(&mut conn).unwrap(),
            AdminTask::RecomputePostChecksums => post::recompute_checksums(&mut conn).unwrap(),
            AdminTask::RegenerateThumbnail => post::regenerate_thumbnail(&mut conn).unwrap(),
        }
    }
    parsed_arguments.len()
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

fn error_message() -> String {
    let possible_arguments: Vec<&'static str> = AdminTask::iter().map(AdminTask::into).collect();
    format!("Command line arguments should be one of {possible_arguments:?}")
}
