mod post;

use std::str::FromStr;
use strum::EnumString;

pub fn run_tasks() -> usize {
    let parsed_arguments: Vec<_> = std::env::args()
        .filter_map(|arg| AdminTask::from_str(&arg).ok())
        .collect();

    let mut conn = crate::get_connection().unwrap();
    for &arg in parsed_arguments.iter() {
        match arg {
            AdminTask::RenamePostContent => post::rename_post_content().unwrap(),
            AdminTask::RecomputePostSignatures => post::recompute_signatures(&mut conn).unwrap(),
            AdminTask::RecomputePostSignatureIndexes => post::recompute_indexes(&mut conn).unwrap(),
            AdminTask::RecomputePostChecksums => post::recompute_checksums(&mut conn).unwrap(),
        }
    }
    parsed_arguments.len()
}

#[derive(Clone, Copy, EnumString)]
#[strum(serialize_all = "snake_case")]
enum AdminTask {
    RenamePostContent,
    RecomputePostSignatures,
    RecomputePostSignatureIndexes,
    RecomputePostChecksums,
}
