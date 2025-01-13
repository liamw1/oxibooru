use crate::content::hash;
use crate::model::comment::Comment;
use crate::model::enums::{AvatarStyle, UserRank};
use crate::model::post::{Post, PostFavorite, PostScore};
use crate::model::user::User;
use crate::resource;
use crate::schema::{comment, post, post_favorite, post_score, user};
use crate::time::DateTime;
use diesel::dsl::count;
use diesel::prelude::*;
use serde::Serialize;
use serde_with::skip_serializing_none;
use std::str::FromStr;
use strum::{EnumString, EnumTable};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MicroUser {
    name: String,
    avatar_url: String,
}

impl MicroUser {
    pub fn new(name: String, avatar_style: AvatarStyle) -> Self {
        let avatar_url = match avatar_style {
            AvatarStyle::Gravatar => hash::gravatar_url(&name),
            AvatarStyle::Manual => hash::custom_avatar_url(&name),
        };
        Self { name, avatar_url }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    Full,
    PublicOnly,
}

#[derive(Clone, Copy, EnumString, EnumTable)]
#[strum(serialize_all = "camelCase")]
pub enum Field {
    Version,
    Name,
    Email,
    Rank,
    LastLoginTime,
    CreationTime,
    AvatarStyle,
    AvatarUrl,
    CommentCount,
    UploadedPostCount,
    LikedPostCount,
    DislikedPostCount,
    FavoritePostCount,
}

impl Field {
    pub fn create_table(fields_str: &str) -> Result<FieldTable<bool>, <Self as FromStr>::Err> {
        let mut table = FieldTable::filled(false);
        let fields = fields_str
            .split(',')
            .map(Self::from_str)
            .collect::<Result<Vec<_>, _>>()?;
        for field in fields.into_iter() {
            table[field] = true;
        }
        Ok(table)
    }
}

#[skip_serializing_none]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserInfo {
    version: Option<DateTime>,
    name: Option<String>,
    email: Option<PrivateData<Option<String>>>,
    rank: Option<UserRank>,
    last_login_time: Option<DateTime>,
    creation_time: Option<DateTime>,
    avatar_style: Option<AvatarStyle>,
    avatar_url: Option<String>,
    comment_count: Option<i64>,
    uploaded_post_count: Option<i64>,
    liked_post_count: Option<PrivateData<i64>>,
    disliked_post_count: Option<PrivateData<i64>>,
    favorite_post_count: Option<i64>,
}

impl UserInfo {
    pub fn new(
        conn: &mut PgConnection,
        user: User,
        fields: &FieldTable<bool>,
        visibility: Visibility,
    ) -> QueryResult<Self> {
        let mut user_info = Self::new_batch(conn, vec![user], fields, visibility)?;
        assert_eq!(user_info.len(), 1);
        Ok(user_info.pop().unwrap())
    }

    pub fn new_from_id(
        conn: &mut PgConnection,
        user_id: i32,
        fields: &FieldTable<bool>,
        visibility: Visibility,
    ) -> QueryResult<Self> {
        let mut user_info = Self::new_batch_from_ids(conn, vec![user_id], fields, visibility)?;
        assert_eq!(user_info.len(), 1);
        Ok(user_info.pop().unwrap())
    }

    pub fn new_batch(
        conn: &mut PgConnection,
        users: Vec<User>,
        fields: &FieldTable<bool>,
        visibility: Visibility,
    ) -> QueryResult<Vec<Self>> {
        let batch_size = users.len();

        let mut comment_counts = fields[Field::CommentCount]
            .then(|| get_comment_counts(conn, &users))
            .transpose()?
            .unwrap_or_default();
        resource::check_batch_results(comment_counts.len(), batch_size);

        let mut upload_counts = fields[Field::UploadedPostCount]
            .then(|| get_uploaded_post_counts(conn, &users))
            .transpose()?
            .unwrap_or_default();
        resource::check_batch_results(upload_counts.len(), batch_size);

        let mut like_counts = fields[Field::LikedPostCount]
            .then(|| get_liked_post_counts(conn, &users, visibility))
            .transpose()?
            .unwrap_or_default();
        resource::check_batch_results(like_counts.len(), batch_size);

        let mut dislike_counts = fields[Field::DislikedPostCount]
            .then(|| get_disliked_post_counts(conn, &users, visibility))
            .transpose()?
            .unwrap_or_default();
        resource::check_batch_results(dislike_counts.len(), batch_size);

        let mut favorite_counts = fields[Field::FavoritePostCount]
            .then(|| get_favorite_post_counts(conn, &users))
            .transpose()?
            .unwrap_or_default();
        resource::check_batch_results(favorite_counts.len(), batch_size);

        let results = users
            .into_iter()
            .rev()
            .map(|user| Self {
                avatar_url: fields[Field::AvatarUrl].then(|| user.avatar_url()),
                version: fields[Field::Version].then_some(user.last_edit_time),
                name: fields[Field::Name].then_some(user.name),
                email: fields[Field::Email].then_some(match visibility {
                    Visibility::Full => PrivateData::Expose(user.email),
                    Visibility::PublicOnly => PrivateData::Visible(false),
                }),
                rank: fields[Field::Rank].then_some(user.rank),
                last_login_time: fields[Field::LastLoginTime].then_some(user.last_login_time),
                creation_time: fields[Field::CreationTime].then_some(user.creation_time),
                avatar_style: fields[Field::AvatarStyle].then_some(user.avatar_style),
                comment_count: comment_counts.pop(),
                uploaded_post_count: upload_counts.pop(),
                liked_post_count: like_counts.pop(),
                disliked_post_count: dislike_counts.pop(),
                favorite_post_count: favorite_counts.pop(),
            })
            .collect::<Vec<_>>();
        Ok(results.into_iter().rev().collect())
    }

    pub fn new_batch_from_ids(
        conn: &mut PgConnection,
        user_ids: Vec<i32>,
        fields: &FieldTable<bool>,
        visibility: Visibility,
    ) -> QueryResult<Vec<Self>> {
        let unordered_users = user::table.filter(user::id.eq_any(&user_ids)).load(conn)?;
        let users = resource::order_as(unordered_users, &user_ids);
        Self::new_batch(conn, users, fields, visibility)
    }
}

#[derive(Clone, Serialize)]
#[serde(untagged)]
enum PrivateData<T> {
    Expose(T),
    Visible(bool), // Set to false to indicate hidden
}

fn get_comment_counts(conn: &mut PgConnection, users: &[User]) -> QueryResult<Vec<i64>> {
    Comment::belonging_to(users)
        .group_by(comment::user_id)
        .select((comment::user_id.assume_not_null(), count(comment::user_id)))
        .load(conn)
        .map(|comment_counts| {
            resource::order_like(comment_counts, users, |&(id, _)| id)
                .into_iter()
                .map(|comment_count| comment_count.map(|(_, count)| count).unwrap_or(0))
                .collect()
        })
}

fn get_uploaded_post_counts(conn: &mut PgConnection, users: &[User]) -> QueryResult<Vec<i64>> {
    Post::belonging_to(users)
        .group_by(post::user_id)
        .select((post::user_id.assume_not_null(), count(post::user_id)))
        .load(conn)
        .map(|upload_counts| {
            resource::order_like(upload_counts, users, |&(id, _)| id)
                .into_iter()
                .map(|upload_count| upload_count.map(|(_, count)| count).unwrap_or(0))
                .collect()
        })
}

fn get_liked_post_counts(
    conn: &mut PgConnection,
    users: &[User],
    visibility: Visibility,
) -> QueryResult<Vec<PrivateData<i64>>> {
    if visibility == Visibility::PublicOnly {
        return Ok(vec![PrivateData::Visible(false); users.len()]);
    }

    PostScore::belonging_to(users)
        .group_by(post_score::user_id)
        .select((post_score::user_id, count(post_score::user_id)))
        .filter(post_score::score.eq(1))
        .load(conn)
        .map(|like_counts| {
            resource::order_like(like_counts, users, |&(id, _)| id)
                .into_iter()
                .map(|like_count| like_count.map(|(_, count)| count).unwrap_or(0))
                .map(PrivateData::Expose)
                .collect()
        })
}

fn get_disliked_post_counts(
    conn: &mut PgConnection,
    users: &[User],
    visibility: Visibility,
) -> QueryResult<Vec<PrivateData<i64>>> {
    if visibility == Visibility::PublicOnly {
        return Ok(vec![PrivateData::Visible(false); users.len()]);
    }

    PostScore::belonging_to(users)
        .group_by(post_score::user_id)
        .select((post_score::user_id, count(post_score::user_id)))
        .filter(post_score::score.eq(-1))
        .load(conn)
        .map(|dislike_counts| {
            resource::order_like(dislike_counts, users, |&(id, _)| id)
                .into_iter()
                .map(|dislike_count| dislike_count.map(|(_, count)| count).unwrap_or(0))
                .map(PrivateData::Expose)
                .collect()
        })
}

fn get_favorite_post_counts(conn: &mut PgConnection, users: &[User]) -> QueryResult<Vec<i64>> {
    PostFavorite::belonging_to(users)
        .group_by(post_favorite::user_id)
        .select((post_favorite::user_id, count(post_favorite::user_id)))
        .load(conn)
        .map(|favorite_counts| {
            resource::order_like(favorite_counts, users, |&(id, _)| id)
                .into_iter()
                .map(|favorite_count| favorite_count.map(|(_, count)| count).unwrap_or(0))
                .collect()
        })
}
