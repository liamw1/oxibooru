use crate::api::ApiResult;
use crate::auth::password;
use crate::content::hash;
use crate::schema::user;
use crate::{api, config, db};
use argon2::password_hash::SaltString;
use diesel::prelude::*;
use lettre::message::header::ContentType;
use lettre::message::Mailbox;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, SmtpTransport, Transport};
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use warp::{Filter, Rejection, Reply};

pub fn routes() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    let request_reset = warp::get()
        .and(warp::path!("password-reset" / String))
        .map(request_reset)
        .map(api::Reply::from);
    let reset_password = warp::post()
        .and(warp::path!("password-reset" / String))
        .and(warp::body::json())
        .map(reset_password)
        .map(api::Reply::from);

    request_reset.or(reset_password)
}

fn get_user_info(conn: &mut PgConnection, identifier: &str) -> ApiResult<(i32, String, Option<String>, String)> {
    user::table
        .select((user::id, user::name, user::email, user::password_salt))
        .filter(user::name.eq(identifier).or(user::email.eq(identifier)))
        .first(conn)
        .map_err(api::Error::from)
}

fn request_reset(identifier: String) -> ApiResult<()> {
    let smtp_info = config::smtp().ok_or(api::Error::MissingSmtpInfo)?;
    let identifier = percent_encoding::percent_decode_str(&identifier).decode_utf8()?;

    let mut conn = db::get_connection()?;
    let (_id, username, user_email, password_salt) = get_user_info(&mut conn, &identifier)?;
    let user_email_address = user_email.ok_or(api::Error::NoEmail)?;
    let user_mailbox: Mailbox = format!("User <{user_email_address}>").parse()?;

    let reset_token = hash::compute_url_safe_hash(&password_salt);
    let domain = config::get().domain.as_deref().unwrap_or("");
    let url = format!("{domain}/password-reset/{username}/?token={reset_token}");

    let email = Message::builder()
        .from(smtp_info.from.clone())
        .to(user_mailbox)
        .subject("Password Reset Request")
        .header(ContentType::TEXT_PLAIN)
        .body(format!(
            "Hello,
        
             You (or someone else) requested to reset your password on {}.\n
             If you wish to proceed, click this link: {url}\n
             Otherwise, please ignore this email.",
            config::get().public_info.name
        ))?;
    let credentials = Credentials::new(smtp_info.username.clone(), smtp_info.password.clone());

    // Open a remote connection to gmail
    let mailer = SmtpTransport::relay("smtp.gmail.com")
        .unwrap()
        .credentials(credentials)
        .build();

    // Send the email
    match mailer.send(&email) {
        Ok(_) => Ok(()),
        Err(err) => Err(api::Error::FailedEmailTransport(err.to_string())),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ResetToken {
    token: String,
}

#[derive(Serialize)]
struct NewPassword {
    password: String,
}

fn reset_password(identifier: String, confirmation: ResetToken) -> ApiResult<NewPassword> {
    let identifier = percent_encoding::percent_decode_str(&identifier).decode_utf8()?;

    db::get_connection()?.transaction(|conn| {
        let (user_id, _name, _email, password_salt) = get_user_info(conn, &identifier)?;
        if confirmation.token != hash::compute_url_safe_hash(&password_salt) {
            return Err(api::Error::UnauthorizedPasswordReset);
        }

        // TODO: Create random alphanumeric password instead of numeric
        let temporary_password = OsRng.next_u64().to_string();

        let salt = SaltString::generate(&mut OsRng);
        let hash = password::hash_password(&temporary_password, &salt)?;
        diesel::update(user::table.find(user_id))
            .set((user::password_salt.eq(salt.as_str()), user::password_hash.eq(hash)))
            .execute(conn)?;

        Ok(NewPassword {
            password: temporary_password,
        })
    })
}
