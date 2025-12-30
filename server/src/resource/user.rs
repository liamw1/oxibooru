use crate::config::Config;
use crate::content::hash;
use crate::get_user_stats;
use crate::model::enums::{AvatarStyle, Score, UserRank};
use crate::model::post::PostScore;
use crate::model::user::User;
use crate::resource::{self, BoolFill};
use crate::schema::{post_score, user};
use crate::string::SmallString;
use crate::time::DateTime;
use diesel::dsl::count_star;
use diesel::{BelongingToDsl, ExpressionMethods, Identifiable, PgConnection, QueryDsl, QueryResult, RunQueryDsl};
use serde::Serialize;
use serde_with::skip_serializing_none;
use server_macros::non_nullable_options;
use strum::{EnumString, EnumTable};
use utoipa::ToSchema;

/// A user resource stripped down to `name` and `avatarUrl` fields.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MicroUser {
    /// The user name.
    #[schema(examples("username"))]
    name: SmallString,
    /// The URL to the avatar.
    #[schema(examples("https://gravatar.com/avatar/60602eb3c4f?d=retro&s=300"))]
    avatar_url: String,
}

impl MicroUser {
    pub fn new(config: &Config, name: SmallString, avatar_style: AvatarStyle) -> Self {
        let avatar_url = match avatar_style {
            AvatarStyle::Gravatar => hash::gravatar_url(config, &name),
            AvatarStyle::Manual => config.custom_avatar_url(&name),
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

impl BoolFill for FieldTable<bool> {
    fn filled(val: bool) -> Self {
        Self::filled(val)
    }
}

/// A single user.
#[non_nullable_options]
#[skip_serializing_none]
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserInfo {
    /// Resource version. See [versioning](#Versioning).
    version: Option<DateTime>,
    /// The user name.
    name: Option<SmallString>,
    /// The user email. It is available only if the request is authenticated by the same user,
    /// or the authenticated user can change the email. If it's unavailable, the server returns `false`.
    /// If the user hasn't specified an email, the server returns `null`.
    email: Option<PrivateData<Option<SmallString>>>,
    /// The user rank, which effectively affects their privileges.
    rank: Option<UserRank>,
    /// The last login time.
    last_login_time: Option<DateTime>,
    /// The user registration time.
    creation_time: Option<DateTime>,
    /// How to render the user avatar.
    avatar_style: Option<AvatarStyle>,
    /// The URL to the avatar.
    avatar_url: Option<String>,
    /// Number of comments.
    comment_count: Option<i64>,
    /// Number of uploaded posts.
    uploaded_post_count: Option<i64>,
    /// Nubmer of liked posts. It is available only if the request is authenticated by the same user.
    /// If it's unavailable, the server returns `false`.
    liked_post_count: Option<PrivateData<i64>>,
    /// Number of disliked posts. It is available only if the request is authenticated by the same user.
    /// If it's unavailable, the server returns `false`.
    disliked_post_count: Option<PrivateData<i64>>,
    /// Number of favorited posts.
    favorite_post_count: Option<i64>,
}

impl UserInfo {
    pub fn new(
        conn: &mut PgConnection,
        config: &Config,
        user: User,
        fields: &FieldTable<bool>,
        visibility: Visibility,
    ) -> QueryResult<Self> {
        Self::new_batch(conn, config, vec![user], fields, visibility).map(resource::single)
    }

    pub fn new_from_id(
        conn: &mut PgConnection,
        config: &Config,
        user_id: i64,
        fields: &FieldTable<bool>,
        visibility: Visibility,
    ) -> QueryResult<Self> {
        Self::new_batch_from_ids(conn, config, &[user_id], fields, visibility).map(resource::single)
    }

    pub fn new_batch(
        conn: &mut PgConnection,
        config: &Config,
        users: Vec<User>,
        fields: &FieldTable<bool>,
        visibility: Visibility,
    ) -> QueryResult<Vec<Self>> {
        #[allow(clippy::wildcard_imports)]
        use crate::schema::user_statistics::dsl::*;

        let mut comment_counts =
            resource::retrieve(fields[Field::CommentCount], || get_user_stats!(conn, &users, comment_count))?;
        let mut upload_counts =
            resource::retrieve(fields[Field::UploadedPostCount], || get_user_stats!(conn, &users, upload_count))?;
        let mut like_counts = resource::retrieve(fields[Field::LikedPostCount], || {
            get_post_score_counts(conn, &users, Score::Like, visibility)
        })?;
        let mut dislike_counts = resource::retrieve(fields[Field::DislikedPostCount], || {
            get_post_score_counts(conn, &users, Score::Dislike, visibility)
        })?;
        let mut favorite_counts =
            resource::retrieve(fields[Field::FavoritePostCount], || get_user_stats!(conn, &users, favorite_count))?;

        let batch_size = users.len();
        resource::check_batch_results(batch_size, comment_counts.len());
        resource::check_batch_results(batch_size, upload_counts.len());
        resource::check_batch_results(batch_size, like_counts.len());
        resource::check_batch_results(batch_size, dislike_counts.len());
        resource::check_batch_results(batch_size, favorite_counts.len());

        let results = users
            .into_iter()
            .rev()
            .map(|user| Self {
                avatar_url: fields[Field::AvatarUrl].then(|| user.avatar_url(config)),
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
        config: &Config,
        user_ids: &[i64],
        fields: &FieldTable<bool>,
        visibility: Visibility,
    ) -> QueryResult<Vec<Self>> {
        let unordered_users = user::table.filter(user::id.eq_any(user_ids)).load(conn)?;
        let users = resource::order_as(unordered_users, user_ids);
        Self::new_batch(conn, config, users, fields, visibility)
    }
}

#[derive(Clone, Serialize, ToSchema)]
#[serde(untagged)]
enum PrivateData<T> {
    Expose(T),
    Visible(bool), // Set to false to indicate hidden
}

fn get_post_score_counts(
    conn: &mut PgConnection,
    users: &[User],
    score: Score,
    visibility: Visibility,
) -> QueryResult<Vec<PrivateData<i64>>> {
    if visibility == Visibility::PublicOnly {
        return Ok(vec![PrivateData::Visible(false); users.len()]);
    }

    PostScore::belonging_to(users)
        .group_by(post_score::user_id)
        .select((post_score::user_id, count_star()))
        .filter(post_score::score.eq(score))
        .load(conn)
        .map(|like_counts| {
            resource::order_like(like_counts, users, |&(id, _)| id)
                .into_iter()
                .map(|like_count| like_count.map_or(0, |(_, count)| count))
                .map(PrivateData::Expose)
                .collect()
        })
}

#[doc(hidden)]
#[macro_export]
macro_rules! get_user_stats {
    ($conn:expr, $users:expr, $column:expr) => {{
        let user_ids: Vec<_> = $users.iter().map(Identifiable::id).copied().collect();
        $crate::schema::user_statistics::table
            .select(($crate::schema::user_statistics::user_id, $column))
            .filter($crate::schema::user_statistics::user_id.eq_any(&user_ids))
            .load($conn)
            .map(|user_stats| {
                resource::order_transformed_as(user_stats, &user_ids, |&(id, _)| id)
                    .into_iter()
                    .map(|(_, stat)| stat)
                    .collect::<Vec<i64>>()
            })
    }};
}
