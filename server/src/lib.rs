pub mod models;
pub mod schema;

use diesel::pg::PgConnection;
use diesel::prelude::*;
use models::NewUser;
use models::User;
use std::result::Result;
use thiserror::Error;

#[derive(Debug, Error)]
#[error(transparent)]
pub enum ConnectionError {
    Dotenvy(#[from] dotenvy::Error),
    EnvVar(#[from] std::env::VarError),
    DieselConnection(#[from] diesel::ConnectionError),
}

#[derive(Debug, Error)]
#[error(transparent)]
pub enum QueryError {
    ServerConnection(#[from] ConnectionError),
    DieselQuery(#[from] diesel::result::Error),
}

pub fn establish_connection() -> Result<PgConnection, ConnectionError> {
    dotenvy::dotenv()?;
    let database_url = std::env::var("DATABASE_URL")?;
    PgConnection::establish(&database_url).map_err(|error| ConnectionError::DieselConnection(error))
}

pub fn create_user(conn: &mut PgConnection, name: &str) -> QueryResult<User> {
    let new_post = NewUser { name };
    diesel::insert_into(schema::users::table)
        .values(new_post)
        .returning(User::as_returning())
        .get_result(conn)
}
