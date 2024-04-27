use diesel::prelude::*;
use server::models::User;
use server::schema::users;

fn print_users() -> Result<(), server::QueryError> {
    let mut connection = server::establish_connection()?;
    let results = users::table
        .limit(5)
        .select(User::as_select())
        .load(&mut connection)?;

    println!("Displaying {} users", results.len());
    for user in results {
        println!("ID: {}", user.id);
        println!("Name: {}", user.name);
        println!("Creation Time: {}", user.creation_time);
        println!("Last Login: {}", user.last_login_time);
        println!("");
    }

    Ok(())
}

fn main() {
    match print_users() {
        Ok(_) => (),
        Err(error) => println!("ERROR: {error}"),
    }
}
