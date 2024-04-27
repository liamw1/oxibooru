use std::string::String;
use thiserror::Error;

#[derive(Debug, Error)]
#[error(transparent)]
pub enum WriteError {
    StdIO(#[from] std::io::Error),
    ServerQuery(#[from] server::QueryError),
}

fn upload_user_to_database(username: &String) -> Result<(), server::QueryError> {
    let connection = &mut server::establish_connection()?;
    let user = server::create_user(connection, &username)?;
    println!("\nSaved user {username} with id {}", user.id);
    Ok(())
}

fn take_user_input() -> std::io::Result<String> {
    let mut username = String::new();
    println!("What would you like your username to be?");
    std::io::stdin().read_line(&mut username)?;
    let username = username.trim_end().to_owned(); // Remove the trailing newline
    Ok(username)
}

fn write_user() -> Result<(), WriteError> {
    let username = take_user_input()?;
    upload_user_to_database(&username)?;
    Ok(())
}

fn main() {
    match write_user() {
        Ok(_) => (),
        Err(error) => println!("ERROR: {error}"),
    }
}
