use crate::app::{AppState, Context};
use crate::config::{Action, RegexType};
use crate::extract::Ctx;
use crate::web::Tab;
use askama::Template;
use axum::response::Html;
use axum::{Router, routing};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/login", routing::get(login))
        .route("/register", routing::get(register))
        .route("/password-reset", routing::get(password_reset))
}

#[derive(Template)]
#[template(path = "pages/registration.html")]
struct RegistrationTemplate {
    ctx: Context,
    active_tab: Tab,
}

async fn register(ctx: Ctx) -> Html<String> {
    let Ctx(ctx, _) = ctx;
    RegistrationTemplate {
        ctx,
        active_tab: Tab::Account,
    }
    .render()
    .map(Html)
    .unwrap()
}

#[derive(Template)]
#[template(path = "pages/login.html")]
struct LoginTemplate {
    ctx: Context,
    active_tab: Tab,
}

async fn login(ctx: Ctx) -> Html<String> {
    let Ctx(ctx, _) = ctx;
    LoginTemplate {
        ctx,
        active_tab: Tab::Login,
    }
    .render()
    .map(Html)
    .unwrap()
}

#[derive(Template)]
#[template(path = "pages/password_reset.html")]
struct PasswordResetTemplate {
    ctx: Context,
    active_tab: Tab,
}

async fn password_reset(ctx: Ctx) -> Html<String> {
    let Ctx(ctx, _) = ctx;
    PasswordResetTemplate {
        ctx,
        active_tab: Tab::Login,
    }
    .render()
    .map(Html)
    .unwrap()
}
