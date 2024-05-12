pub mod func;
pub mod model;
pub mod query;
pub mod schema;
#[cfg(test)]
mod test;
pub mod util;

use diesel::prelude::*;
use model::privilege::UserPrivilege;
use model::user::{NewUser, User};
use std::result::Result;
use thiserror::Error;

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

fn delete_users(conn: &mut PgConnection, pattern: &str) -> QueryResult<usize> {
    use schema::user;
    let num_deleted = diesel::delete(user::table.filter(user::columns::name.like(pattern))).execute(conn)?;

    Ok(num_deleted)
}

fn print_users(conn: &mut PgConnection) {
    let query_result = schema::user::table.limit(5).select(User::as_select()).load(conn);

    let users = match query_result {
        Ok(users) => users,
        Err(error) => return println!("{error}"),
    };

    println!("Displaying {} users", users.len());
    for user in users {
        println!("ID: {}", user.id);
        println!("Name: {}", user.name);
        println!("Creation Time: {}", user.creation_time);
        println!("Last Login: {}", user.last_login_time);
        println!("");
    }
}

pub fn write_user(conn: &mut PgConnection, name: &str) -> QueryResult<User> {
    let new_user = NewUser {
        name,
        password_hash: "password",
        password_salt: "salt",
        rank: UserPrivilege::Regular,
    };
    diesel::insert_into(schema::user::table)
        .values(new_user)
        .returning(User::as_returning())
        .get_result(conn)
}

fn create_user(conn: &mut PgConnection, name: &str) -> QueryResult<User> {
    let new_user = NewUser {
        name,
        password_hash: "password",
        password_salt: "salt",
        rank: UserPrivilege::Regular,
    };
    diesel::insert_into(schema::user::table)
        .values(new_user)
        .returning(User::as_returning())
        .get_result(conn)
}

fn request_user_input() -> std::io::Result<String> {
    let mut buffer = String::new();
    std::io::stdin().read_line(&mut buffer)?;
    Ok(buffer.trim_end().to_owned())
}

fn main() {
    let mut conn = match establish_connection() {
        Ok(conn) => conn,
        Err(err) => return println!("{err}"),
    };

    println!("What command would you like to run?");
    let command = match request_user_input() {
        Ok(input) => input,
        Err(err) => return println!("{err}"),
    };

    match command.as_str() {
        "show" => return print_users(&mut conn),
        "create" => (),
        "delete" => (),
        _ => return println!("Invalid command. Valid commands are 'show', 'create', 'delete'."),
    }

    if command.as_str() == "show" {
        return print_users(&mut conn);
    }

    println!("Please type the user's name.");
    let username = match request_user_input() {
        Ok(input) => input,
        Err(err) => return println!("{err}"),
    };

    let error = match command.as_str() {
        "create" => create_user(&mut conn, username.as_str()).err(),
        "delete" => delete_users(&mut conn, username.as_str()).err(),
        _ => return,
    };
    match error {
        Some(err) => println!("{err}"),
        None => (),
    }
}
