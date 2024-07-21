use crate::api::{ApiResult, AuthResult};
use crate::resource::tag_category::TagCategoryInfo;
use crate::{api, config};
use serde::Serialize;
use warp::{Filter, Rejection, Reply};

pub fn routes() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    let list_tag_categories = warp::get()
        .and(warp::path!("tag-categories"))
        .and(api::auth())
        .map(list_tag_categories)
        .map(api::Reply::from);

    list_tag_categories
}

#[derive(Serialize)]
struct TagCategoryList {
    results: Vec<TagCategoryInfo>,
}

fn list_tag_categories(auth: AuthResult) -> ApiResult<TagCategoryList> {
    let client = auth?;
    api::verify_privilege(client.as_ref(), config::privileges().tag_category_list)?;

    let mut conn = crate::establish_connection()?;
    Ok(TagCategoryList {
        results: TagCategoryInfo::all(&mut conn)?,
    })
}
