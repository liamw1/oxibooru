use crate::app::AppState;
use crate::config::Config;
use crate::content::hash::PostHash;
use crate::model::enums::MimeType;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::Redirect;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use hmac::{Mac, SimpleHmac};
use utoipa_axum::router::OpenApiRouter;

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().route("/legacy/redirect/{filename}", axum::routing::get(redirect))
}

async fn redirect(State(state): State<AppState>, Path(filename): Path<String>) -> Result<Redirect, StatusCode> {
    let (post_id, tail) = filename.split_once('_').ok_or(StatusCode::UNPROCESSABLE_ENTITY)?;
    let post_id = post_id.parse().ok().ok_or(StatusCode::UNPROCESSABLE_ENTITY)?;
    let (post_hash, extension) = tail.split_once('.').ok_or(StatusCode::UNPROCESSABLE_ENTITY)?;
    let mime_type = MimeType::from_extension(extension)
        .ok()
        .ok_or(StatusCode::UNSUPPORTED_MEDIA_TYPE)?;

    if post_hash != legacy_url_hash(&state.config, post_id) {
        return Err(StatusCode::NOT_FOUND);
    }

    let new_post_hash = PostHash::new(&state.config, post_id);
    let new_url = format!("/{}", new_post_hash.content_url(mime_type));
    Ok(Redirect::permanent(&new_url))
}

type Hmac = SimpleHmac<blake3::Hasher>;

fn legacy_url_hash(config: &Config, post_id: i64) -> String {
    let mut mac = Hmac::new_from_slice(config.content_secret.as_bytes()).expect("HMAC should take key of any size");
    mac.update(&post_id.to_le_bytes());
    let hash = mac.finalize();
    URL_SAFE_NO_PAD.encode(hash.into_bytes())
}
