use diesel::prelude::*;
use diesel::result::{DatabaseErrorKind, Error};

pub fn validate_update(table_name: &str, rows_updated: usize) -> QueryResult<()> {
    validate_uniqueness(table_name, "update", rows_updated)
}

pub fn validate_deletion(table_name: &str, rows_deleted: usize) -> QueryResult<()> {
    validate_uniqueness(table_name, "delete", rows_deleted)
}

fn validate_uniqueness(table_name: &str, transaction_type: &str, rows_changed: usize) -> QueryResult<()> {
    let error_message =
        |msg: String| -> Error { Error::DatabaseError(DatabaseErrorKind::UniqueViolation, Box::new(msg)) };
    match rows_changed {
        0 => Err(error_message(format!("Failed to {transaction_type} {table_name}: no entry found"))),
        1 => Ok(()),
        _ => Err(error_message(format!("Failed to {transaction_type} {table_name}: entry is not unique"))),
    }
}
