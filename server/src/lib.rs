pub mod models;
pub mod schema;

use diesel::pg::PgConnection;
use diesel::prelude::*;
use models::{NewUser, User};
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

pub fn create_user(connection: &mut PgConnection, name: &str) -> QueryResult<User> {
    let current_time = chrono::Utc::now();
    let new_user = NewUser {
        name,
        password_hash: "password",
        rank: "new_user",
        creation_time: current_time,
        last_login_time: current_time,
    };
    diesel::insert_into(schema::users::table)
        .values(new_user)
        .returning(User::as_returning())
        .get_result(connection)
}
