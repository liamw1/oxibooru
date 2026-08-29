use crate::api::error::ApiError;
use crate::app::AppState;
use crate::config::Action;
use crate::extract::Ctx;
use crate::web::{Html, Tab, WebError};
use askama::Template;
use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use std::sync::Arc;
use std::time::Duration;

pub async fn slow(State(state): State<AppState>, req: Request, next: Next) -> Response {
    const DELAY: Duration = Duration::from_secs(2);
    if state.config.args.slow {
        tokio::time::sleep(DELAY).await;
    }
    next.run(req).await
}

pub async fn convert_error(ctx: Ctx, req: Request, next: Next) -> Response {
    let response = next.run(req).await;

    let error_string = if let Some(err) = response.extensions().get::<Arc<ApiError>>() {
        Some(err.to_string())
    } else if let Some(err) = response.extensions().get::<Arc<WebError>>() {
        Some(err.to_string())
    } else {
        None
    };
    if let Some(error) = error_string {
        let html = ErrorTemplate {
            ctx,
            active_tab: Tab::None,
            error,
        }
        .render()
        .map(Html)
        .expect("Error template should never fail");

        // Override target in case error happens when client expects HTML fragment
        ([("HX-Retarget", "body"), ("HX-Reswap", "innerHTML")], html).into_response()
    } else {
        response
    }
}

#[derive(Template)]
#[template(path = "pages/error.html")]
struct ErrorTemplate {
    ctx: Ctx,
    active_tab: Tab,
    error: String,
}
