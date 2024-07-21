use crate::auth::content;
use crate::model::comment::Comment;
use crate::model::enums::{AvatarStyle, UserRank};
use crate::model::post::{Post, PostFavorite, PostScore};
use crate::model::user::{User, UserId};
use crate::resource;
use crate::schema::{comment, post, post_favorite, post_score};
use crate::util::DateTime;
use diesel::dsl::*;
use diesel::prelude::*;
use serde::Serialize;
use std::str::FromStr;
use strum::{EnumString, EnumTable};

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

    pub fn new2(name: String, avatar_style: AvatarStyle) -> Self {
        let avatar_url = match avatar_style {
            AvatarStyle::Gravatar => content::gravatar_url(&name),
            AvatarStyle::Manual => content::custom_avatar_url(&name),
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
            .into_iter()
            .map(Self::from_str)
            .collect::<Result<Vec<_>, _>>()?;
        for field in fields.into_iter() {
            table[field] = true;
        }
        Ok(table)
    }
}

// TODO: Remove renames by changing references to these names in client
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

    pub fn new_batch(
        conn: &mut PgConnection,
        mut users: Vec<User>,
        fields: &FieldTable<bool>,
        visibility: Visibility,
    ) -> QueryResult<Vec<Self>> {
        let batch_size = users.len();

        let mut comment_counts = fields[Field::CommentCount]
            .then_some(get_comment_counts(conn, &users)?)
            .unwrap_or_default();
        resource::check_batch_results(comment_counts.len(), batch_size);

        let mut upload_counts = fields[Field::UploadedPostCount]
            .then_some(get_uploaded_post_counts(conn, &users)?)
            .unwrap_or_default();
        resource::check_batch_results(upload_counts.len(), batch_size);

        let mut like_counts = fields[Field::LikedPostCount]
            .then_some(get_liked_post_counts(conn, &users, visibility)?)
            .unwrap_or_default();
        resource::check_batch_results(like_counts.len(), batch_size);

        let mut dislike_counts = fields[Field::DislikedPostCount]
            .then_some(get_disliked_post_counts(conn, &users, visibility)?)
            .unwrap_or_default();
        resource::check_batch_results(dislike_counts.len(), batch_size);

        let mut favorite_counts = fields[Field::FavoritePostCount]
            .then_some(get_favorite_post_counts(conn, &users)?)
            .unwrap_or_default();
        resource::check_batch_results(favorite_counts.len(), batch_size);

        let mut results: Vec<Self> = Vec::new();
        while let Some(user) = users.pop() {
            results.push(Self {
                avatar_url: fields[Field::AvatarUrl].then_some(user.avatar_url()),
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
            });
        }
        Ok(results.into_iter().rev().collect())
    }
}

#[derive(Clone, Serialize)]
#[serde(untagged)]
enum PrivateData<T> {
    Expose(T),
    Visible(bool), // Set to false to indicate hidden
}

fn get_comment_counts(conn: &mut PgConnection, users: &[User]) -> QueryResult<Vec<i64>> {
    let comment_counts: Vec<(UserId, i64)> = Comment::belonging_to(users)
        .group_by(comment::user_id)
        .select((comment::user_id, count(comment::user_id)))
        .load(conn)?;
    Ok(comment_counts
        .grouped_by(users)
        .into_iter()
        .map(|counts| counts.first().map(|(_, count)| *count).unwrap_or(0))
        .collect())
}

fn get_uploaded_post_counts(conn: &mut PgConnection, users: &[User]) -> QueryResult<Vec<i64>> {
    let upload_counts: Vec<(UserId, i64)> = Post::belonging_to(users)
        .group_by(post::user_id)
        .select((post::user_id.assume_not_null(), count(post::user_id)))
        .load(conn)?;
    Ok(upload_counts
        .grouped_by(users)
        .into_iter()
        .map(|counts| counts.first().map(|(_, count)| *count).unwrap_or(0))
        .collect())
}

fn get_liked_post_counts(
    conn: &mut PgConnection,
    users: &[User],
    visibility: Visibility,
) -> QueryResult<Vec<PrivateData<i64>>> {
    if visibility == Visibility::PublicOnly {
        return Ok(vec![PrivateData::Visible(false); users.len()]);
    }

    let like_counts: Vec<(UserId, i64)> = PostScore::belonging_to(users)
        .group_by(post_score::user_id)
        .select((post_score::user_id, count(post_score::user_id)))
        .filter(post_score::score.eq(1))
        .load(conn)?;
    Ok(like_counts
        .grouped_by(users)
        .into_iter()
        .map(|counts| counts.first().map(|(_, count)| *count).unwrap_or(0))
        .map(PrivateData::Expose)
        .collect())
}

fn get_disliked_post_counts(
    conn: &mut PgConnection,
    users: &[User],
    visibility: Visibility,
) -> QueryResult<Vec<PrivateData<i64>>> {
    if visibility == Visibility::PublicOnly {
        return Ok(vec![PrivateData::Visible(false); users.len()]);
    }

    let dislike_counts: Vec<(UserId, i64)> = PostScore::belonging_to(users)
        .group_by(post_score::user_id)
        .select((post_score::user_id, count(post_score::user_id)))
        .filter(post_score::score.eq(-1))
        .load(conn)?;
    Ok(dislike_counts
        .grouped_by(users)
        .into_iter()
        .map(|counts| counts.first().map(|(_, count)| *count).unwrap_or(0))
        .map(PrivateData::Expose)
        .collect())
}

fn get_favorite_post_counts(conn: &mut PgConnection, users: &[User]) -> QueryResult<Vec<i64>> {
    let favorite_counts: Vec<(UserId, i64)> = PostFavorite::belonging_to(users)
        .group_by(post_favorite::user_id)
        .select((post_favorite::user_id, count(post_favorite::user_id)))
        .load(conn)?;
    Ok(favorite_counts
        .grouped_by(users)
        .into_iter()
        .map(|counts| counts.first().map(|(_, count)| *count).unwrap_or(0))
        .collect())
}
