use diesel::prelude::*;
use diesel::result::{DatabaseErrorKind, Error};

pub fn validate_uniqueness(table_name: &str, num_deleted: usize) -> QueryResult<()> {
    let error_message =
        |msg: String| -> Error { Error::DatabaseError(DatabaseErrorKind::UniqueViolation, Box::new(msg)) };
    match num_deleted {
        0 => Err(error_message(format!("Failed to delete {table_name}: no entry found"))),
        1 => Ok(()),
        _ => Err(error_message(format!("Failed to delete {table_name}: entry is not unique"))),
    }
}
