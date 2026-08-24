use crate::app::{AppState, Context};
use crate::config::Action;
use crate::extract::{Ctx, Json, Offset, Query, ResourceParams};
use crate::resource::user::{Field, UserInfo};
use crate::web::Tab;
use crate::web::pager::{Page, Pager};
use crate::{api, time};
use askama::Template;
use axum::response::Html;
use axum::{Router, routing};
use serde::{Deserialize, Serialize};
use std::num::NonZeroU64;

pub fn routes() -> Router<AppState> {
    Router::new().route("/users", routing::get(list))
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
