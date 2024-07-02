use crate::auth::content;
use crate::model::pool::Pool;
use crate::model::post::Post;
use crate::model::tag::Tag;
use crate::model::user::User;
use crate::schema::{pool_category, tag_category};
use diesel::prelude::*;
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MicroUser {
    name: String,
    avatar_url: String,
}

impl MicroUser {
    pub fn new(user: User) -> Self {
        let avatar_url = user.avatar_url();
        Self {
            name: user.name,
            avatar_url,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MicroTag {
    names: Vec<String>,
    category: String,
    usages: i64,
}

impl MicroTag {
    pub fn new(conn: &mut PgConnection, tag: Tag) -> QueryResult<Self> {
        Ok(MicroTag {
            names: tag.names(conn)?,
            category: tag_category::table
                .find(tag.category_id)
                .select(tag_category::name)
                .first(conn)?,
            usages: tag.usages(conn)?,
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MicroPool {
    id: i32,
    names: Vec<String>,
    category: String,
    description: Option<String>,
    post_count: i64,
}

impl MicroPool {
    pub fn new(conn: &mut PgConnection, pool: Pool) -> QueryResult<Self> {
        let names = pool.names(conn)?;
        let category = pool_category::table
            .find(pool.category_id)
            .select(pool_category::name)
            .first(conn)?;
        let post_count = pool.post_count(conn)?;

        Ok(MicroPool {
            id: pool.id,
            names,
            category,
            description: pool.description,
            post_count,
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MicroPost {
    id: i32,
    thumbnail_url: String,
}

impl MicroPost {
    pub fn new(post: &Post) -> Self {
        MicroPost {
            id: post.id,
            thumbnail_url: content::post_thumbnail_url(post.id),
        }
    }
}
