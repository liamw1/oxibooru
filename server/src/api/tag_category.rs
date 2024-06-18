use crate::api::ApiError;

// pub fn get_tag_categories() -> Result<impl Reply, Rejection> {}

struct TagCategoryInfo {
    version: i32,
    name: String,
    color: String,
    usages: i64,
    order: i32,
    default: bool,
}

fn collect_tag_categories() -> Result<TagCategoryInfo, ApiError> {
    let header = warp::header::optional::<String>("Authorization");

    let mut conn = crate::establish_connection()?;
    let info = TagCategoryInfo {
        version: 0,
        name: String::new(),
        color: String::new(),
        usages: 0,
        order: 0,
        default: false,
    };

    Ok(info)
}
