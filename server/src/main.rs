pub mod model;
pub mod schema;

use diesel::prelude::*;
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
    PgConnection::establish(&database_url).map_err(|error| ConnectionError::DieselConnection(error))
}

fn delete_users(connection: &mut PgConnection, pattern: &str) -> QueryResult<usize> {
    use schema::user;
    let num_deleted = diesel::delete(user::table.filter(user::columns::name.like(pattern)))
        .execute(connection)?;

    Ok(num_deleted)
}

fn print_users(connection: &mut PgConnection) {
    let query_result = schema::user::table
        .limit(5)
        .select(User::as_select())
        .load(connection);

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

pub fn write_user(connection: &mut PgConnection, name: &str) -> QueryResult<User> {
    let current_time = chrono::Utc::now();
    let new_user = NewUser {
        name,
        password_hash: "password",
        rank: "new_user",
        creation_time: current_time,
        last_login_time: current_time,
    };
    diesel::insert_into(schema::user::table)
        .values(new_user)
        .returning(User::as_returning())
        .get_result(connection)
}

fn create_user(connection: &mut PgConnection, name: &str) -> QueryResult<User> {
    let current_time = chrono::Utc::now();
    let new_user = NewUser {
        name,
        password_hash: "password",
        rank: "new_user",
        creation_time: current_time,
        last_login_time: current_time,
    };
    diesel::insert_into(schema::user::table)
        .values(new_user)
        .returning(User::as_returning())
        .get_result(connection)
}

fn request_user_input() -> std::io::Result<String> {
    let mut buffer = String::new();
    std::io::stdin().read_line(&mut buffer)?;
    Ok(buffer.trim_end().to_owned())
}

fn main() {
    let mut connection = match establish_connection() {
        Ok(connection) => connection,
        Err(error) => return println!("{error}"),
    };

    println!("What command would you like to run?");
    let command = match request_user_input() {
        Ok(input) => input,
        Err(error) => return println!("{error}"),
    };

    match command.as_str() {
        "show" => return print_users(&mut connection),
        "create" => (),
        "delete" => (),
        _ => return println!("Invalid command. Valid commands are 'show', 'create', 'delete'."),
    }

    if command.as_str() == "show" {
        return print_users(&mut connection);
    }

    println!("Please type the user's name.");
    let username = match request_user_input() {
        Ok(input) => input,
        Err(error) => return println!("{error}"),
    };

    let error = match command.as_str() {
        "create" => create_user(&mut connection, username.as_str()).err(),
        "delete" => delete_users(&mut connection, username.as_str()).err(),
        _ => return,
    };
    match error {
        Some(error) => println!("{error}"),
        None => (),
    }
}
