use crate::string::SmallString;
use diesel::deserialize::QueryableByName;
use diesel::sql_types::{Array, Text};
use diesel::{PgConnection, QueryResult, RunQueryDsl};
use strum::Display;

pub mod pool;
pub mod post;
pub mod tag;
pub mod user;

// NOTE: Unless documented otherwise, the functions in this module do not check
// that the client has the required privileges to perform their respective actions.
// Make sure to check for privileges before calling them, if necessary.

#[derive(Display)]
#[strum(serialize_all = "lowercase")]
enum NameType {
    Tag,
    Pool,
}

#[derive(QueryableByName)]
struct NewName {
    #[diesel(sql_type = Text)]
    name: SmallString,
}

/// Gather names that are case-fold distinct from existing names in database.
/// We use a query here because CITEXT semantics differ from comparing
/// `str::to_lowercased`-ed strings in certain cases.
fn get_new_names(conn: &mut PgConnection, names: &[SmallString], name_type: NameType) -> QueryResult<Vec<SmallString>> {
    let query = format!(
        "SELECT DISTINCT ON (unnest::CITEXT) unnest AS name
        FROM unnest($1::text[]) WITH ORDINALITY
        WHERE NOT EXISTS (
            SELECT 1 FROM {name_type}_name WHERE {name_type}_name.name = unnest::CITEXT
        )
        ORDER BY unnest::CITEXT, ordinality"
    );
    diesel::sql_query(query)
        .bind::<Array<Text>, _>(names)
        .load::<NewName>(conn)
        .map(|rows| rows.into_iter().map(|row| row.name).collect())
}
