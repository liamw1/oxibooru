use crate::api::ApiResult;
use crate::auth::password;
use crate::content::hash;
use crate::schema::user;
use crate::string::SmallString;
use crate::{api, config, db};
use argon2::password_hash::SaltString;
use argon2::password_hash::rand_core::{OsRng, RngCore};
use axum::extract::Path;
use axum::{Json, Router, routing};
use diesel::prelude::*;
use lettre::message::Mailbox;
use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, SmtpTransport, Transport};
use serde::{Deserialize, Serialize};

pub fn routes() -> Router {
    Router::new().route("/password-reset/{identifier}", routing::get(request_reset).post(reset_password))
}

fn get_user_info(
    conn: &mut PgConnection,
    identifier: &str,
) -> ApiResult<(i64, SmallString, Option<SmallString>, String)> {
    user::table
        .select((user::id, user::name, user::email, user::password_salt))
        .filter(user::name.eq(identifier).or(user::email.eq(identifier)))
        .first(conn)
        .map_err(api::Error::from)
}

async fn request_reset(Path(identifier): Path<String>) -> ApiResult<Json<()>> {
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
    let credentials = Credentials::new(smtp_info.username.to_string(), smtp_info.password.to_string());

    // Open a remote connection to gmail
    let mailer = SmtpTransport::relay("smtp.gmail.com")
        .unwrap()
        .credentials(credentials)
        .build();

    // Send the email
    mailer.send(&email).map(|_| Json(())).map_err(api::Error::from)
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

async fn reset_password(
    Path(identifier): Path<String>,
    Json(confirmation): Json<ResetToken>,
) -> ApiResult<Json<NewPassword>> {
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

        Ok(Json(NewPassword {
            password: temporary_password,
        }))
    })
}
