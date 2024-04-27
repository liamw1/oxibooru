use diesel::prelude::*;
use server::models::User;
use thiserror::Error;

#[derive(Debug, Error)]
#[error(transparent)]
pub enum ReadError {
    ParseInt(#[from] std::num::ParseIntError),
    #[error("{0}")]
    OutOfRange(&'static str),
}

#[derive(Debug, Error)]
#[error(transparent)]
pub enum GetUserError {
    Read(#[from] ReadError),
    Connection(#[from] server::ConnectionError),
}

fn get_user_id() -> Result<i32, ReadError> {
    let user_id = std::env::args()
        .nth(1)
        .ok_or(ReadError::OutOfRange("get_user requires a user id"))?
        .parse::<i32>()?;

    Ok(user_id)
}

fn user_info() -> Result<String, GetUserError> {
    use server::schema::users::dsl::users;

    let user_id = get_user_id()?;
    let mut connection = server::establish_connection()?;

    let user = users
        .find(user_id)
        .select(User::as_select())
        .first(&mut connection)
        .optional();

    return match user {
        Ok(Some(user)) => Ok(format!("User with id: {} has name: {}", user.id, user.name)),
        Ok(None) => Ok(format!("Unable to find user {user_id}")),
        Err(_) => Ok(format!("An error occured while fetching name {user_id}")),
    };
}

fn main() {
    match user_info() {
        Ok(info) => println!("{info}"),
        Err(error) => println!("ERROR: {error}"),
    }
}
