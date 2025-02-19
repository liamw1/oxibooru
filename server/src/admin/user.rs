use crate::api::ApiResult;
use crate::auth::password;
use crate::config::RegexType;
use crate::schema::user;
use crate::{admin, api, db};
use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::SaltString;
use diesel::dsl::exists;
use diesel::prelude::*;

/// This function prompts the user for input again to reset passwords for specific users.
pub fn reset_password() -> ApiResult<()> {
    let mut conn = db::get_connection()?;
    let mut user_buffer = String::new();
    let mut password_buffer = String::new();
    loop {
        println!("Please enter the username of the user you would like to reset a password for. Enter \"done\" when finished.");
        let user = admin::prompt_user_input("Username", &mut user_buffer);
        if user == "done" {
            break;
        }

        let user_exists: bool =
            diesel::select(exists(user::table.filter(user::name.eq(user)))).get_result(&mut conn)?;
        if !user_exists {
            eprintln!("ERROR: No user with this username exists\n");
            continue;
        }

        let password = admin::prompt_user_input("New password", &mut password_buffer);
        if password == "done" {
            break;
        }
        if let Err(err) = api::verify_matches_regex(password, RegexType::Password) {
            eprintln!("ERROR: {err}\n");
            continue;
        }

        let salt = SaltString::generate(&mut OsRng);
        let hash = password::hash_password(password, &salt)?;
        diesel::update(user::table)
            .filter(user::name.eq(user))
            .set((user::password_hash.eq(&hash), user::password_salt.eq(salt.as_str())))
            .execute(&mut conn)?;
        println!("Password reset successful.\n");
    }
    Ok(())
}
