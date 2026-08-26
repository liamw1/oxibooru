use crate::config::Action;
use crate::extract::Ctx;
use crate::web::{Html, Tab, WebError};
use askama::Template;
use axum::extract::Request;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use std::sync::Arc;

pub async fn convert_error(ctx: Ctx, req: Request, next: Next) -> Response {
    let response = next.run(req).await;

    if let Some(err) = response.extensions().get::<Arc<WebError>>() {
        ErrorTemplate {
            ctx,
            active_tab: Tab::None,
            error: &err,
        }
        .render()
        .map(Html)
        .expect("Error template should never fail")
        .into_response()
    } else {
        response
    }
}

#[derive(Template)]
#[template(path = "pages/error.html")]
struct ErrorTemplate<'a> {
    ctx: Ctx,
    active_tab: Tab,
    error: &'a WebError,
}
