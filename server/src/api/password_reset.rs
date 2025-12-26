use crate::api::error::{ApiError, ApiResult};
use crate::api::extract::{Json, Path};
use crate::app::AppState;
use crate::auth::password;
use crate::content::hash;
use crate::model::enums::ResourceType;
use crate::schema::user;
use crate::string::SmallString;
use argon2::password_hash::SaltString;
use argon2::password_hash::rand_core::{OsRng, RngCore};
use axum::extract::State;
use axum::{Router, routing};
use diesel::{
    BoolExpressionMethods, Connection, ExpressionMethods, OptionalExtension, PgConnection, QueryDsl, RunQueryDsl,
};
use lettre::Address;
use lettre::message::Mailbox;
use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, SmtpTransport, Transport};
use percent_encoding::NON_ALPHANUMERIC;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

pub fn routes() -> Router<AppState> {
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
        .optional()?
        .ok_or(ApiError::NotFound(ResourceType::User))
}

/// See [request-password-reset](https://github.com/liamw1/oxibooru/blob/master/docs/API.md#request-password-reset)
async fn request_reset(State(state): State<AppState>, Path(identifier): Path<String>) -> ApiResult<Json<()>> {
    let smtp_info = state.config.smtp().ok_or(ApiError::MissingSmtpInfo)?;

    let mut conn = state.get_connection()?;
    let (_id, username, user_email, password_salt) = get_user_info(&mut conn, &identifier)?;
    let user_email = user_email.ok_or(ApiError::NoEmail)?;
    let user_mailbox = Mailbox::new(None, Address::from_str(&user_email)?);

    let domain = if let Some(domain) = state.config.domain.as_deref() {
        domain.to_string()
    } else if let Ok(domain) = std::env::var("HTTP_ORIGIN") {
        domain
    } else if let Ok(domain) = std::env::var("HTTP_REFERER") {
        domain
    } else if let Ok(port) = std::env::var("PORT") {
        format!("http://localhost:{port}")
    } else {
        String::new()
    };
    let domain = domain.trim_end_matches('/');

    let site_name = &state.config.public_info.name;
    let username = percent_encoding::utf8_percent_encode(&username, NON_ALPHANUMERIC);
    let separator = percent_encoding::percent_encode_byte(b':');
    let reset_token = hash::compute_url_safe_hash(password_salt.as_bytes());
    let url = format!("{domain}/password-reset/{username}{separator}{reset_token}");

    let email = Message::builder()
        .from(smtp_info.from.clone())
        .to(user_mailbox)
        .subject(format!("Password reset for {site_name}"))
        .header(ContentType::TEXT_HTML)
        .body(format!(
            "<html>
                <body>
                    <p>Hello,</p>
                    <p>You (or someone else) requested to reset your password on {site_name}.<br>
                    If you wish to proceed, click this link: <a href=\"{url}\">{url}</a></p>
                    <p>Otherwise, please ignore this email.</p>
                </body>
            </html>"
        ))?;

    // Open a remote connection to SMTP relay
    let mut smtp_builder = SmtpTransport::relay(&smtp_info.host)?;
    if let (Some(smtp_username), Some(smtp_password)) = (smtp_info.username.as_ref(), smtp_info.password.as_ref()) {
        let credentials = Credentials::new(smtp_username.to_string(), smtp_password.to_string());
        smtp_builder = smtp_builder.credentials(credentials);
    }
    if let Some(port) = smtp_info.port {
        smtp_builder = smtp_builder.port(port);
    }
    let mailer = smtp_builder.build();

    mailer.send(&email).map(|_| Json(())).map_err(ApiError::from)
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

/// Creates a random sequence of printable ASCII characters of the given `length`.
fn generate_temporary_password(length: u8) -> String {
    const NUM_CHARACTERS: u8 = b'~' - b'!';

    let rng = &mut OsRng;
    (0..length)
        .map(|_| b'!' + u8::try_from(rng.next_u32() % u32::from(NUM_CHARACTERS)).expect("NUM_CHARACTERS is <= u8::MAX"))
        .map(char::from)
        .collect()
}

/// See [confirm-password-reset](https://github.com/liamw1/oxibooru/blob/master/docs/API.md#confirm-password-reset)
async fn reset_password(
    State(state): State<AppState>,
    Path(username): Path<String>,
    Json(confirmation): Json<ResetToken>,
) -> ApiResult<Json<NewPassword>> {
    const TEMPORARY_PASSWORD_LENGTH: u8 = 16;

    state.get_connection()?.transaction(|conn| {
        let (user_id, _name, _email, password_salt) = get_user_info(conn, &username)?;
        if confirmation.token != hash::compute_url_safe_hash(password_salt.as_bytes()) {
            return Err(ApiError::UnauthorizedPasswordReset);
        }

        let temporary_password = generate_temporary_password(TEMPORARY_PASSWORD_LENGTH);

        let salt = SaltString::generate(&mut OsRng);
        let hash = password::hash_password(&state.config, &temporary_password, &salt)?;
        diesel::update(user::table.find(user_id))
            .set((user::password_salt.eq(salt.as_str()), user::password_hash.eq(hash)))
            .execute(conn)?;

        Ok(Json(NewPassword {
            password: temporary_password,
        }))
    })
}

#[cfg(test)]
mod test {
    use crate::api::error::ApiResult;
    use crate::test::*;
    use serial_test::parallel;

    #[tokio::test]
    #[parallel]
    async fn error() -> ApiResult<()> {
        verify_response("GET /password-reset/no_one", "password_reset/request_for_nonexistent_user").await?;
        verify_response("POST /password-reset/nobody", "password_reset/confirm_for_nonexistent_user").await?;

        verify_response("GET /password-reset/moderator", "password_reset/no_email").await?;
        verify_response("GET /password-reset/restricted_user", "password_reset/invalid_email").await?;
        verify_response("GET /password-reset/regular_user", "password_reset/reset_disabled").await?;
        verify_response("POST /password-reset/regular_user", "password_reset/invalid_token").await?;
        verify_response("POST /password-reset/regular_user", "password_reset/missing_token").await?;
        Ok(())
    }
}
