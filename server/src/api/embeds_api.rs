use crate::api::{ApiResult, AppState, ResourceParams};
use crate::auth::Client;
use crate::model::enums::ResourceType;
use crate::resource::post::{FieldTable, PostInfo};
use crate::schema::post;
use crate::{api, config, db, resource};
use axum::extract::{Path, Query, State};
use axum::response::Html;
use axum::{routing, Extension, Json, Router};
use diesel::dsl::exists;
use diesel::{Connection, QueryDsl, RunQueryDsl};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_with::skip_serializing_none;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/oembed", routing::get(get_oembed))
        .route("/index/post/{post_id}", routing::get(get_post))
}

#[skip_serializing_none]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Embed {
    version: String,
    #[serde(rename = "type")]
    embed_type: String,
    title: String,
    author_name: Option<String>,
    provider_name: String,
    provider_url: String,
    thumbnail_url: String,
    thumbnail_width: u32,
    thumbnail_height: u32,
    url: String,
    width: u32,
    height: u32,
}
#[derive(Deserialize)]
struct OEmbed {
    url: Option<String>
}

// todo check permissions

fn get_post_info(client: Client, post_id: i64, fields: &FieldTable<bool>) -> Json<PostInfo> {
    let a = db::get_connection().unwrap().transaction(|conn| {
        let post_exists: bool = diesel::select(exists(post::table.find(post_id))).get_result(conn)?;
        if !post_exists {
            return Err(api::Error::NotFound(ResourceType::Post));
        }
        PostInfo::new_from_id(conn, client, post_id, &fields)
            .map(Json)
            .map_err(api::Error::from)
    });

    a.unwrap()
}

fn get_embed(post_info: &Json<PostInfo>) -> Embed {
    Embed {
        version: "1.0".to_string(),
        embed_type: "photo".to_string(),
        title: format!("{} - Post #{}", config::get().public_info.name, post_info.id.unwrap()),
        // todo
        author_name: None,
        provider_name: config::get().public_info.name.to_string(),
        provider_url: config::get().domain.as_deref().unwrap().to_string(),
        thumbnail_url: format!("{}/{}", config::get().domain.as_deref().unwrap(), post_info.thumbnail_url.clone().unwrap()),
        thumbnail_width: config::get().thumbnails.post_width,
        thumbnail_height: config::get().thumbnails.post_height,
        url: format!("{}/{}", config::get().domain.as_deref().unwrap(), post_info.thumbnail_url.clone().unwrap()),
        width: config::get().thumbnails.post_width,
        height: config::get().thumbnails.post_height,
    }
}

async fn get_oembed(Extension(client): Extension<Client>, Query(params): Query<ResourceParams>, Query(url): Query<OEmbed>) -> ApiResult<Json<Embed>> {
    let re = Regex::new(r".*?/post/(?P<post_id>\d+)").unwrap();
    // this will throw a very unhelpful error if the post_id is missing
    if let Some(caps) = re.captures(&url.url.unwrap()) {
        if let Some(post_id) = caps.name("post_id") {
            let fields = resource::create_table(params.fields()).map_err::<Box<dyn std::error::Error>, _>(Box::from).unwrap();
            let post_info = get_post_info(client, post_id.as_str().parse::<i64>()?, &fields);
            return Ok(Json(get_embed(&post_info)))
        }
    }

    return Err(api::Error::NotFound(ResourceType::Post));
}

async fn get_post(State(state): State<AppState>, Extension(client): Extension<Client>, Path(post_id): Path<i64>, Query(params): Query<ResourceParams>) -> Html<String> {
    let fields = resource::create_table(params.fields()).map_err::<Box<dyn std::error::Error>, _>(Box::from).unwrap();
    let post_info = db::get_connection().unwrap().transaction(|conn| {
        let post_exists: bool = diesel::select(exists(post::table.find(post_id))).get_result(conn)?;
        if !post_exists {
            return Err(api::Error::NotFound(ResourceType::Post));
        }
        PostInfo::new_from_id(conn, client, post_id, &fields)
            .map(Json)
            .map_err(api::Error::from)
    }).unwrap();

    // let post_info = get_post_info(client, post_id, &fields);
    let embed = get_embed(&post_info);
    let url = format!("{}/post/{}", config::get().domain.as_deref().clone().unwrap(), post_id);
    let meta = format!(
        r#"
    <meta property="og:site_name" content="{site_name}">
    <meta property="og:url" content="{url}">
    <meta property="og:type" content="article">
    <meta property="og:title" content="{title}">
    <meta name="twitter:title" content="{title}">
    <meta name="twitter:card" content="summary_large_image">
    <meta name="twitter:image" content="{image_url}">
    <meta property="og:image:url" content="{image_url}">
    <meta property="og:image:width" content="{image_width}">
    <meta property="og:image:height" content="{image_height}">
    <meta property="article:author" content="{author}">
    <link rel="alternate" type="application/json+oembed" href="{site_url}/api/oembed?url={encoded_url}" title="{site_title}">
    </head>
    "#,
        site_name = html_escape::encode_text(&embed.provider_name),
        url = url,
        title = html_escape::encode_text(&embed.title),
        image_url = html_escape::encode_text(&embed.url),
        image_width = embed.width,
        image_height = embed.height,
        // todo
        author = "",
        site_url = config::get().domain.as_deref().unwrap(),
        encoded_url = html_escape::encode_text(&url),
        site_title = html_escape::encode_text(&config::get().public_info.name.to_string()),
    );

    let new_html = state.index_htm.unwrap().clone().replace("</head>", &meta)
        .replace("<html>", r#"<html prefix="og: http://ogp.me/ns#">"#)
        .replace("<title>Loading...</title>", &format!("<title>{}</title>", &embed.title));
    Html(new_html)
}