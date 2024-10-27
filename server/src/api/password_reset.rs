use crate::api::ApiResult;
use crate::schema::user;
use crate::{api, config, db};
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

fn request_reset(identifier: String) -> ApiResult<()> {
    let smtp_info = config::smtp().ok_or(api::Error::MissingSmtpInfo)?;
    let identifier = percent_encoding::percent_decode_str(&identifier).decode_utf8()?;

    let mut conn = db::get_connection()?;
    let user_email: Option<String> = user::table
        .select(user::email)
        .filter(user::name.eq(identifier))
        .first(&mut conn)?;
    let email_address = user_email.ok_or(api::Error::NoEmail)?;
    let mailbox: Mailbox = format!("User <{email_address}>").parse()?;

    let reset_token = OsRng.next_u32() % 1_000_000;
    let email = Message::builder()
        .from(smtp_info.from.clone())
        .to(mailbox)
        .subject("Password Reset Request")
        .header(ContentType::TEXT_PLAIN)
        .body(format!(
            "Hello,
        
        We received a request to change your password.
        You can use the code {reset_token} to reset your password.
        If you did not request a password change, you can ignore this message
        and continue to use your current password."
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
    unimplemented!()
}
