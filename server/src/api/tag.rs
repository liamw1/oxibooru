use crate::api;
use crate::api::AuthResult;
use warp::{Filter, Rejection, Reply};

pub fn routes() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    let list_tags = warp::get()
        .and(warp::path!("tags"))
        .and(api::auth())
        .and(warp::query())
        .map(list_tags)
        .map(api::Reply::from);

    list_tags
}

fn list_tags(_auth_result: AuthResult, _query_info: api::PagedQuery) -> Result<(), api::Error> {
    unimplemented!()
}
