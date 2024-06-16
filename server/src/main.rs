pub mod api;
pub mod config;
pub mod func;
pub mod image;
pub mod math;
pub mod model;
pub mod query;
pub mod schema;
#[cfg(test)]
mod test;
pub mod util;

use diesel::prelude::*;
use std::result::Result;
use thiserror::Error;
use warp::Filter;

#[derive(Debug, Error)]
#[error(transparent)]
pub enum ConnectionError {
    Dotenvy(#[from] dotenvy::Error),
    EnvVar(#[from] std::env::VarError),
    DieselConnection(#[from] diesel::ConnectionError),
}

pub fn establish_connection() -> Result<PgConnection, ConnectionError> {
    dotenvy::dotenv()?;
    let database_url = std::env::var("DATABASE_URL")?;
    PgConnection::establish(&database_url).map_err(ConnectionError::DieselConnection)
}

#[tokio::main]
async fn main() {
    println!("hello world!");

    // GET /hello/warp => 200 OK with body "Hello, warp!"
    let hello = warp::path!("hello" / String).map(|name| format!("Hello, {}!", name));

    warp::serve(hello).run(([127, 0, 0, 1], 3030)).await;
}
