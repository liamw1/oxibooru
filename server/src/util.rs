use crate::model::TableName;
use diesel::associations::HasTable;
use diesel::pg::Pg;
use diesel::prelude::*;
use diesel::query_builder::{AsQuery, IntoUpdateTarget, QueryFragment, QueryId, UpdateStatement};
use diesel::result::{DatabaseErrorKind, Error};
use std::ops::Deref;

pub fn delete<R>(conn: &mut PgConnection, row: R) -> QueryResult<()>
where
    R: Deref + IntoUpdateTarget,
    <<R as HasTable>::Table as QuerySource>::FromClause: QueryFragment<Pg>,
    <R as IntoUpdateTarget>::WhereClause: QueryFragment<Pg>,
    <R as IntoUpdateTarget>::WhereClause: QueryId,
    <R as HasTable>::Table: QueryId,
    <R as HasTable>::Table: 'static,
    R::Target: TableName,
{
    conn.transaction(|conn| validate_uniqueness(R::Target::table_name(), "delete", diesel::delete(row).execute(conn)?))
}

pub fn update_single_row<R, T, V>(conn: &mut PgConnection, row: R, values: V) -> QueryResult<()>
where
    R: Deref + IntoUpdateTarget<Table = T>,
    <<R as HasTable>::Table as QuerySource>::FromClause: QueryFragment<Pg>,
    <R as IntoUpdateTarget>::WhereClause: QueryFragment<Pg>,
    V: diesel::AsChangeset<Target = T>,
    T: diesel::QuerySource + diesel::Table,
    UpdateStatement<T, <R as IntoUpdateTarget>::WhereClause, <V as AsChangeset>::Changeset>: AsQuery,
    <V as diesel::AsChangeset>::Changeset: QueryFragment<Pg>,
    R::Target: TableName,
{
    conn.transaction(|conn| {
        validate_uniqueness(R::Target::table_name(), "update", diesel::update(row).set::<V>(values).execute(conn)?)
    })
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
