use crate::admin::LoopState;
use crate::auth::password;
use crate::config::RegexType;
use crate::schema::user;
use crate::{admin, api};
use argon2::password_hash::SaltString;
use argon2::password_hash::rand_core::OsRng;
use diesel::dsl::exists;
use diesel::{ExpressionMethods, PgConnection, QueryDsl, RunQueryDsl};

/// This function prompts the user for input again to reset passwords for specific users.
pub fn reset_password(conn: &mut PgConnection) {
    admin::user_input_loop(conn, |conn: &mut PgConnection, buffer: &mut String| {
        println!(
            "Please enter the username of the user you would like to reset a password for. Enter \"done\" when finished."
        );
        let user = admin::prompt_user_input("Username", buffer).to_owned();
        if let Ok(state) = LoopState::try_from(user.as_str()) {
            return Ok(state);
        }

        // Check if user exists
        match diesel::select(exists(user::table.filter(user::name.eq(&user)))).get_result(conn) {
            Ok(true) => (),
            Ok(false) => return Err(String::from("No user with this username exists")),
            Err(err) => return Err(format!("Could not determine if user exists for reason: {err}")),
        }

        let password = admin::prompt_user_input("New password", buffer);
        if let Ok(state) = LoopState::try_from(password) {
            return Ok(state);
        }
        api::verify_matches_regex(password, RegexType::Password).map_err(|err| err.to_string())?;

        let salt = SaltString::generate(&mut OsRng);
        let hash = password::hash_password(password, &salt)
            .map_err(|err| format!("Could not hash password for reason: {err}"))?;
        diesel::update(user::table)
            .filter(user::name.eq(user))
            .set((user::password_hash.eq(&hash), user::password_salt.eq(salt.as_str())))
            .execute(conn)
            .map_err(|err| format!("Could not update password for reason: {err}"))?;

        println!("Password reset successful.\n");
        Ok(LoopState::Continue)
    });
}
