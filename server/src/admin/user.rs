use crate::admin::input::{self, UserEditor};
use crate::api;
use crate::app::AppState;
use crate::auth::password;
use crate::config::RegexType;
use crate::schema::user;
use argon2::password_hash::SaltString;
use argon2::password_hash::rand_core::OsRng;
use diesel::dsl::exists;
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};

/// This function prompts the user for input again to reset passwords for specific users.
pub fn reset_password(state: &AppState, editor: &mut UserEditor) {
    input::user_input_loop(state, editor, |state: &AppState, editor: &mut UserEditor| {
        println!(
            "Please enter the username of the user you would like to reset a password for. Enter \"done\" when finished."
        );
        let user = input::read("Username: ", editor)?;

        // Check if user exists
        let mut conn = state
            .get_connection()
            .map_err(|err| format!("Could not establish a connection to the database for reason: {err}"))?;
        match diesel::select(exists(user::table.filter(user::name.eq(&user)))).first(&mut conn) {
            Ok(true) => (),
            Ok(false) => return Err("No user with this username exists".into()),
            Err(err) => return Err(format!("Could not determine if user exists for reason: {err}").into()),
        }

        let password = input::read("New password: ", editor)?;
        api::verify_matches_regex(&state.config, &password, RegexType::Password)?;

        let salt = SaltString::generate(&mut OsRng);
        let hash = password::hash_password(&state.config, &password, &salt)
            .map_err(|err| format!("Could not hash password for reason: {err}"))?;
        diesel::update(user::table)
            .filter(user::name.eq(user))
            .set((user::password_hash.eq(&hash), user::password_salt.eq(salt.as_str())))
            .execute(&mut conn)
            .map_err(|err| format!("Could not update password for reason: {err}"))?;

        println!("Password reset successful.\n");
        Ok(())
    });
}
