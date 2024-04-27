use diesel::prelude::*;
use thiserror::Error;

#[derive(Debug, Error)]
#[error(transparent)]
pub enum DeletionError {
    Connection(#[from] server::ConnectionError),
    DieselQuery(#[from] diesel::result::Error),
    #[error("{0}")]
    OutOfRange(&'static str),
}

fn delete_users() -> Result<usize, DeletionError> {
    use server::schema::users;

    let target = std::env::args().nth(1).ok_or(DeletionError::OutOfRange(
        "Expected a target to match against",
    ))?;
    let pattern = format!("%{}%", target);

    let mut connection = server::establish_connection()?;
    let num_deleted = diesel::delete(users::table.filter(users::columns::name.like(pattern)))
        .execute(&mut connection)?;

    Ok(num_deleted)
}

fn main() {
    match delete_users() {
        Ok(num_deleted) => println!("Deleted {} users", num_deleted),
        Err(error) => println!("ERROR: {error}"),
    }
}
