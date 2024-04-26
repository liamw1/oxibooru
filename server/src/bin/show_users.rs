use diesel::prelude::*;
use server::models::User;
use server::schema::users::dsl::users;

fn print_users() -> Result<(), server::QueryError> {
    let mut connection = server::establish_connection()?;
    let results = users
        .limit(5)
        .select(User::as_select())
        .load(&mut connection)?;

    println!("Displaying {} users", results.len());
    for user in results {
        println!("{}", user.id);
        println!("-----------\n");
        println!("{}", user.name);
    }

    Ok(())
}

fn main() {
    match print_users() {
        Ok(_) => (),
        Err(error) => println!("ERROR: {error}"),
    }
}
