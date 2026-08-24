use crate::app::{AppState, Context};
use crate::config::Action;
use crate::extract::{Ctx, Json, Offset, Path, Query, ResourceParams};
use crate::resource::user::{Field, UserInfo};
use crate::string::SmallString;
use crate::web::Tab;
use crate::web::pager::{Page, Pager};
use crate::{api, time};
use askama::Template;
use axum::response::Html;
use axum::{Router, routing};
use serde::{Deserialize, Serialize};
use std::num::NonZeroU64;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/users", routing::get(list))
        .route("/user/{name}", routing::get(summary))
        .route("/user/{name}/edit", routing::get(edit))
        .route("/user/{name}/list-tokens", routing::get(tokens))
        .route("/user/{name}/delete", routing::get(delete))
}

const LIMIT: NonZeroU64 = NonZeroU64::new(30).unwrap();

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct Params {
    search_text: Option<String>,
}

impl Params {
    fn search_text(&self) -> &str {
        self.search_text.as_deref().unwrap_or("")
    }

    fn simplify(mut self) -> Self {
        if self.search_text.as_ref().is_some_and(String::is_empty) {
            self.search_text = None;
        }
        self
    }
}

#[derive(Template)]
#[template(path = "pages/users_page.html")]
struct ListTemplate<'a> {
    ctx: Context,
    active_tab: Tab,
    users: Vec<UserInfo>,
    pager: Pager<'a, Params>,
    params: &'a Params,
}

async fn list(ctx: Ctx, Query(params): Query<Params>, Query(offset): Query<Offset>) -> Html<String> {
    let fields = [Field::Name, Field::LastLoginTime, Field::CreationTime, Field::AvatarUrl].into();

    let query = params.search_text.clone();
    let resource_params = Query(ResourceParams { query, fields });
    let page_params = Query(offset.to_page_params(LIMIT));
    let Json(response) = api::user::list(ctx.clone(), resource_params, page_params)
        .await
        .unwrap();

    let params = params.simplify();
    let pager = Pager::build("users", &params, page_params, response.total);

    let Ctx(ctx, _) = ctx;
    ListTemplate {
        ctx,
        active_tab: Tab::Post,
        users: response.results,
        pager,
        params: &params,
    }
    .render()
    .map(Html)
    .unwrap()
}

#[derive(PartialEq, Eq)]
enum UserTab {
    Summary,
    Edit,
    Tokens,
    Delete,
}

#[derive(Template)]
#[template(path = "pages/user.html")]
struct PoolTemplate {
    ctx: Context,
    active_tab: Tab,
    active_user_tab: UserTab,
    user: UserInfo,
}

async fn view(ctx: Ctx, path: Path<SmallString>, active_user_tab: UserTab) -> Html<String> {
    let fields = [Field::Name].into();

    let resource_params = Query(ResourceParams { query: None, fields });
    let Json(user) = api::user::get(ctx.clone(), path, resource_params).await.unwrap();

    let Ctx(ctx, _) = ctx;
    PoolTemplate {
        ctx,
        active_tab: Tab::User,
        active_user_tab,
        user,
    }
    .render()
    .map(Html)
    .unwrap()
}

async fn summary(ctx: Ctx, path: Path<SmallString>) -> Html<String> {
    view(ctx, path, UserTab::Summary).await
}

async fn edit(ctx: Ctx, path: Path<SmallString>) -> Html<String> {
    view(ctx, path, UserTab::Edit).await
}

async fn tokens(ctx: Ctx, path: Path<SmallString>) -> Html<String> {
    view(ctx, path, UserTab::Tokens).await
}

async fn delete(ctx: Ctx, path: Path<SmallString>) -> Html<String> {
    view(ctx, path, UserTab::Delete).await
}
