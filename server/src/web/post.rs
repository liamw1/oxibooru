use crate::api::post;
use crate::app::{AppState, Context};
use crate::config::Action;
use crate::extract::{Ctx, Json, PageParams, Query, ResourceParams};
use crate::model::enums::{PostSafety, PostType};
use crate::resource::post::Field;
use crate::resource::tag::MicroTag;
use crate::web::SearchQuery;
use askama::Template;
use axum::response::Html;
use axum::{Router, routing};
use strum::IntoEnumIterator;

pub fn routes() -> Router<AppState> {
    Router::new().route("/posts", routing::get(gallery))
}

struct PostInfo {
    id: i64,
    tags: Vec<MicroTag>,
    title: String,
    url: String,
    thumbnail_url: String,
    type_: PostType,
    safety: PostSafety,
    score: i64,
    favorite_count: i64,
    comment_count: i64,
}

#[derive(Template)]
#[template(path = "pages/post_gallery.html")]
struct GalleryTemplate {
    ctx: Context,
    posts: Vec<PostInfo>,
}

async fn gallery(
    ctx: Ctx,
    Query(SearchQuery { query }): Query<SearchQuery>,
    page_params: Query<PageParams>,
) -> Html<String> {
    let fields = [
        Field::Id,
        Field::Tags,
        Field::ThumbnailUrl,
        Field::Type,
        Field::Safety,
        Field::Score,
        Field::FavoriteCount,
        Field::CommentCount,
    ]
    .into();
    let resource_params = Query(ResourceParams { query, fields });
    let Json(response) = post::list(ctx.clone(), resource_params, page_params).await.unwrap();

    let posts = response
        .results
        .into_iter()
        .map(|result| {
            let id = result.id.unwrap();
            PostInfo {
                id,
                tags: result.tags.unwrap(),
                title: String::from("title"),
                url: format!("/post/{id}"),
                thumbnail_url: result.thumbnail_url.unwrap(),
                type_: result.type_.unwrap(),
                safety: result.safety.unwrap(),
                score: result.score.unwrap(),
                favorite_count: result.favorite_count.unwrap(),
                comment_count: result.comment_count.unwrap(),
            }
        })
        .collect();

    let Ctx(ctx, _) = ctx;
    Html(GalleryTemplate { ctx, posts }.render().unwrap())
}
