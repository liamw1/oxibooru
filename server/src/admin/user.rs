use crate::admin::input::{self, UserEditor};
use crate::api;
use crate::app::AppState;
use crate::auth::password;
use crate::config::RegexType;
use crate::schema::user;
use crate::string::SecretString;
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
            .connection_pool
            .get_blocking()
            .map_err(|err| format!("Could not establish a connection to the database for reason: {err}"))?;
        match diesel::select(exists(user::table.filter(user::name.eq(&user)))).first(&mut conn) {
            Ok(true) => (),
            Ok(false) => return Err("No user with this username exists".into()),
            Err(err) => return Err(format!("Could not determine if user exists for reason: {err}").into()),
        }

        let password = input::read("New password: ", editor).map(SecretString::from)?;
        api::verify_matches_regex(&state.config, password.read(), RegexType::Password)?;

        let (hash, salt) = password::hash_password(&state.config, &password)
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
