use crate::app::Context;
use crate::auth::Client;
use crate::config::Config;
use crate::content::hash::PostHash;
use crate::model::comment::Comment;
use crate::model::enums::{AvatarStyle, MimeType, PostFlags, PostSafety, PostType, Rating, Score};
use crate::model::pool::PoolPost;
use crate::model::post::{NewPostNote, Post, PostFavorite, PostNote, PostRelation, PostScore, PostTag};
use crate::model::tag::TagName;
use crate::post_stats;
use crate::resource;
use crate::resource::comment::CommentInfo;
use crate::resource::field::{Batcher, Mask};
use crate::resource::pool::MicroPool;
use crate::resource::tag::MicroTag;
use crate::resource::user::MicroUser;
use crate::schema::{
    comment, comment_score, comment_statistics, pool, pool_category, pool_name, pool_statistics, post, post_favorite,
    post_note, post_relation, post_score, tag, tag_category, tag_name, tag_statistics, user,
};
use crate::string::{LargeString, SmallString};
use crate::time::DateTime;
use diesel::dsl::{exists, not};
use diesel::{
    BelongingToDsl, ExpressionMethods, GroupedBy, Identifiable, NullableExpressionMethods, PgConnection, QueryDsl,
    QueryResult, RunQueryDsl, SelectableHelper,
};
use serde::{Deserialize, Serialize};
use serde_with::skip_serializing_none;
use server_macros::non_nullable_options;
use std::collections::{HashMap, HashSet};
use std::convert::Infallible;
use std::sync::Arc;
use strum::EnumString;
use utoipa::ToSchema;

#[derive(Clone, Serialize, Deserialize, ToSchema)]
pub struct Note {
    #[serde(skip)]
    id: i64,
    /// Where to draw the annotation. Each point must have coordinates within 0 to 1.
    /// For example, `[[0,0],[0,1],[1,1],[1,0]]` will draw the annotation on the whole post,
    /// whereas `[[0,0],[0,0.5],[0.5,0.5],[0.5,0]]` will draw it inside the post's upper left quarter.
    polygon: Vec<[f32; 2]>,
    /// The annotation text. The client should render is as Markdown.
    text: LargeString,
}

impl Note {
    pub fn new(note: PostNote) -> Self {
        const PANIC_MESSAGE: &str = "Polygon array should not contain NULL values";
        let polygon = note
            .polygon
            .chunks_exact(2)
            .map(|vertex| [vertex[0].expect(PANIC_MESSAGE), vertex[1].expect(PANIC_MESSAGE)])
            .collect();
        Self {
            id: note.id,
            polygon,
            text: note.text,
        }
    }

    pub fn to_new_post_note(&'_ self, post_id: i64) -> NewPostNote<'_> {
        NewPostNote {
            post_id,
            polygon: self.polygon.as_flattened(),
            text: &self.text,
        }
    }

    pub fn id(&self) -> i64 {
        self.id
    }
}

/// A post resource stripped down to `id` and `thumbnailUrl` fields.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MicroPost {
    /// The post identifier.
    pub id: i64,
    /// Where the post thumbnail is located.
    pub thumbnail_url: String,
}

#[derive(Clone, Copy, EnumString)]
#[strum(serialize_all = "camelCase")]
pub enum Field {
    Version,
    Id,
    User,
    FileSize,
    CanvasWidth,
    CanvasHeight,
    Safety,
    Type,
    MimeType,
    Checksum,
    ChecksumMd5,
    Flags,
    Source,
    Description,
    CreationTime,
    LastEditTime,
    ContentUrl,
    ThumbnailUrl,
    Tags,
    Comments,
    Relations,
    Pools,
    Notes,
    Score,
    OwnScore,
    OwnFavorite,
    TagCount,
    CommentCount,
    RelationCount,
    NoteCount,
    FavoriteCount,
    FeatureCount,
    LastFeatureTime,
    FavoritedBy,
    HasCustomThumbnail,
}

impl From<Field> for u64 {
    fn from(value: Field) -> Self {
        value as u64
    }
}

/// One file together with its metadata posted to the site.
#[non_nullable_options]
#[skip_serializing_none]
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PostInfo {
    /// Resource version. See [versioning](#Versioning).
    version: Option<DateTime>,
    /// The post identifier.
    id: Option<i64>,
    /// Who created the post.
    #[schema(nullable)]
    user: Option<Option<MicroUser>>,
    /// The size of the file in bytes.
    file_size: Option<i64>,
    /// The original width of the post content.
    canvas_width: Option<i32>,
    /// The original height of the post content.
    canvas_height: Option<i32>,
    /// Whether the post is safe for work.
    safety: Option<PostSafety>,
    /// The type of the post.
    type_: Option<PostType>,
    /// Subsidiary to `<type>`, used to tell exact content format. Useful for `<video>` tags for instance.
    mime_type: Option<MimeType>,
    /// The BLAKE3 file checksum.
    checksum: Option<String>,
    /// The MD5 file checksum.
    #[serde(rename = "checksumMD5")]
    checksum_md5: Option<String>,
    /// Various flags such as whether the post is looped.
    flags: Option<PostFlags>,
    /// Where the post was grabbed form, supplied by the user.
    source: Option<LargeString>,
    /// Text description for the post. The client should render is as Markdown.
    description: Option<LargeString>,
    /// Time the tag was created.
    creation_time: Option<DateTime>,
    /// Time the tag was last edited.
    last_edit_time: Option<DateTime>,
    /// Where the post content is located.
    content_url: Option<String>,
    /// Where the post thumbnail is located.
    thumbnail_url: Option<String>,
    /// List of tags the post is tagged with.
    tags: Option<Vec<MicroTag>>,
    /// List of comments under the post.
    comments: Option<Vec<CommentInfo>>,
    /// List of related posts. Links to related posts are shown to the user by the web client.
    relations: Option<Vec<MicroPost>>,
    /// List of pools the post is a member of.
    pools: Option<Vec<MicroPool>>,
    /// List of post annotations.
    notes: Option<Vec<Note>>,
    /// The collective score (+1/-1 rating) of the given post.
    score: Option<i64>,
    /// The score (+1/-1 rating) of the given post by the authenticated user.
    own_score: Option<Rating>,
    /// Whether the authenticated user has given post in their favorites.
    own_favorite: Option<bool>,
    /// How many tags the post is tagged with.
    tag_count: Option<i64>,
    /// How many comments are filed under the post.
    comment_count: Option<i64>,
    /// How many posts are related to this post.
    relation_count: Option<i64>,
    /// How many notes the post has.
    note_count: Option<i64>,
    /// How many users have the post in their favorites.
    favorite_count: Option<i64>,
    /// How many times has the post been featured.
    feature_count: Option<i64>,
    /// The last time the post was featured.
    #[schema(nullable)]
    last_feature_time: Option<Option<DateTime>>,
    /// List of users that favorited the post.
    favorited_by: Option<Vec<MicroUser>>,
    /// Whether the post uses custom thumbnail.
    has_custom_thumbnail: Option<bool>,
}

impl PostInfo {
    pub fn new(conn: &mut PgConnection, ctx: &Context, post: Post, fields: Mask<Field>) -> QueryResult<Self> {
        Self::new_batch(conn, ctx, vec![post], fields).map(resource::single)
    }

    pub fn new_from_id(conn: &mut PgConnection, ctx: &Context, post_id: i64, fields: Mask<Field>) -> QueryResult<Self> {
        Self::new_batch_from_ids(conn, ctx, &[post_id], fields).map(resource::single)
    }

    pub fn new_batch(
        conn: &mut PgConnection,
        ctx: &Context,
        posts: Vec<Post>,
        fields: Mask<Field>,
    ) -> QueryResult<Vec<Self>> {
        #[allow(clippy::wildcard_imports)]
        use crate::schema::post_statistics::dsl::*;

        let f = Batcher::new(fields, posts.len());
        let mut owners = f.exec(Field::User, || get_owners(conn, &ctx.config, &posts))?;
        let Ok(mut content_urls) = f.exec(Field::ContentUrl, || get_content_urls(&ctx.config, &posts));
        let Ok(mut thumbnail_urls) = f.exec(Field::ThumbnailUrl, || get_thumbnail_urls(&ctx.config, &posts));
        let mut tags = f.exec(Field::Tags, || get_tags(conn, &posts))?;
        let mut comments = f.exec(Field::Comments, || get_comments(conn, ctx, &posts))?;
        let mut relations = f.exec(Field::Relations, || get_relations(conn, ctx, &posts))?;
        let mut pools = f.exec(Field::Pools, || get_pools(conn, &posts))?;
        let mut notes = f.exec(Field::Notes, || get_notes(conn, &posts))?;
        let mut scores = f.exec(Field::Score, || post_stats!(conn, &posts, score, i64))?;
        let mut own_scores = f.exec(Field::OwnScore, || get_own_scores(conn, ctx.client, &posts))?;
        let mut own_favorites = f.exec(Field::OwnFavorite, || get_own_favorites(conn, ctx.client, &posts))?;
        let mut tag_counts = f.exec(Field::TagCount, || post_stats!(conn, &posts, tag_count, i64))?;
        let mut comment_counts = f.exec(Field::CommentCount, || post_stats!(conn, &posts, comment_count, i64))?;
        let mut relation_counts = f.exec(Field::RelationCount, || post_stats!(conn, &posts, relation_count, i64))?;
        let mut note_counts = f.exec(Field::NoteCount, || post_stats!(conn, &posts, note_count, i64))?;
        let mut favorite_counts = f.exec(Field::FavoriteCount, || post_stats!(conn, &posts, favorite_count, i64))?;
        let mut feature_counts = f.exec(Field::FeatureCount, || post_stats!(conn, &posts, feature_count, i64))?;
        let mut last_feature_times =
            f.exec(Field::LastFeatureTime, || post_stats!(conn, &posts, last_feature_time, Option<DateTime>))?;
        let mut favorited_by = f.exec(Field::FavoritedBy, || get_favorited_by(conn, &ctx.config, &posts))?;

        let mut results = posts
            .into_iter()
            .rev()
            .map(|post| Self {
                version: fields[Field::Version].then_some(post.last_edit_time),
                id: fields[Field::Id].then_some(post.id),
                user: owners.pop(),
                file_size: fields[Field::FileSize].then_some(post.file_size),
                canvas_width: fields[Field::CanvasWidth].then_some(post.width),
                canvas_height: fields[Field::CanvasHeight].then_some(post.height),
                safety: fields[Field::Safety].then_some(post.safety),
                type_: fields[Field::Type].then_some(post.type_),
                mime_type: fields[Field::MimeType].then_some(post.mime_type),
                checksum: fields[Field::Checksum].then(|| hex::encode(&post.checksum)),
                checksum_md5: fields[Field::ChecksumMd5].then(|| hex::encode(&post.checksum_md5)),
                flags: fields[Field::Flags].then_some(post.flags),
                source: fields[Field::Source].then_some(post.source),
                description: fields[Field::Description].then_some(post.description),
                creation_time: fields[Field::CreationTime].then_some(post.creation_time),
                last_edit_time: fields[Field::LastEditTime].then_some(post.last_edit_time),
                content_url: content_urls.pop(),
                thumbnail_url: thumbnail_urls.pop(),
                tags: tags.pop(),
                relations: relations.pop(),
                notes: notes.pop(),
                score: scores.pop(),
                own_score: own_scores.pop(),
                own_favorite: own_favorites.pop(),
                tag_count: tag_counts.pop(),
                favorite_count: favorite_counts.pop(),
                comment_count: comment_counts.pop(),
                note_count: note_counts.pop(),
                feature_count: feature_counts.pop(),
                relation_count: relation_counts.pop(),
                last_feature_time: last_feature_times.pop(),
                favorited_by: favorited_by.pop(),
                comments: comments.pop(),
                pools: pools.pop(),
                has_custom_thumbnail: fields[Field::HasCustomThumbnail].then(|| {
                    PostHash::new(&ctx.config, post.id, Some(post.custom_thumbnail_size))
                        .custom_thumbnail_path()
                        .exists()
                }),
            })
            .collect::<Vec<_>>();
        results.reverse();
        Ok(results)
    }

    pub fn new_batch_from_ids(
        conn: &mut PgConnection,
        ctx: &Context,
        post_ids: &[i64],
        fields: Mask<Field>,
    ) -> QueryResult<Vec<Self>> {
        let unordered_posts = post::table.filter(post::id.eq_any(post_ids)).load(conn)?;
        let posts = resource::order_as(unordered_posts, post_ids);
        Self::new_batch(conn, ctx, posts, fields)
    }
}

fn get_owners(conn: &mut PgConnection, config: &Config, posts: &[Post]) -> QueryResult<Vec<Option<MicroUser>>> {
    let post_ids: Vec<_> = posts.iter().map(Identifiable::id).copied().collect();
    post::table
        .filter(post::id.eq_any(&post_ids))
        .inner_join(user::table)
        .select((post::id, user::name, user::avatar_style))
        .load::<(i64, SmallString, AvatarStyle)>(conn)
        .map(|post_info| {
            resource::order_like(post_info, posts, |&(id, ..)| id)
                .into_iter()
                .map(|post_owner| {
                    post_owner.map(|(_, username, avatar_style)| MicroUser::new(config, username, avatar_style))
                })
                .collect()
        })
}

#[allow(clippy::unnecessary_wraps)]
fn get_content_urls(config: &Config, posts: &[Post]) -> Result<Vec<String>, Infallible> {
    Ok(posts
        .iter()
        .map(|post| PostHash::new(config, post.id, Some(post.custom_thumbnail_size)).content_url(post.mime_type))
        .collect())
}

#[allow(clippy::unnecessary_wraps)]
fn get_thumbnail_urls(config: &Config, posts: &[Post]) -> Result<Vec<String>, Infallible> {
    Ok(posts
        .iter()
        .map(|post| PostHash::new(config, post.id, Some(post.custom_thumbnail_size)).thumbnail_url())
        .collect())
}

fn get_tags(conn: &mut PgConnection, posts: &[Post]) -> QueryResult<Vec<Vec<MicroTag>>> {
    let tag_info = tag::table
        .inner_join(tag_statistics::table)
        .inner_join(tag_category::table)
        .inner_join(tag_name::table);
    let post_tags: Vec<(PostTag, i64, i64)> = PostTag::belonging_to(posts)
        .inner_join(tag_info)
        .select((PostTag::as_select(), tag::category_id, tag_statistics::usage_count))
        .filter(TagName::primary())
        .order((tag_category::order, tag_name::name))
        .load(conn)?;
    let tag_ids: HashSet<i64> = post_tags.iter().map(|(post_tag, ..)| post_tag.tag_id).collect();

    let tag_names: Vec<(i64, SmallString)> = tag_name::table
        .select((tag_name::tag_id, tag_name::name))
        .filter(tag_name::tag_id.eq_any(tag_ids))
        .order((tag_name::tag_id, tag_name::order))
        .load(conn)?;
    let names_map = resource::collect_names(tag_names);

    let category_names: HashMap<i64, SmallString> = tag_category::table
        .select((tag_category::id, tag_category::name))
        .load(conn)?
        .into_iter()
        .collect();

    Ok(post_tags
        .grouped_by(posts)
        .into_iter()
        .map(|tags_on_post| {
            tags_on_post
                .into_iter()
                .map(|(post_tag, category_id, usages)| MicroTag {
                    names: Arc::clone(&names_map[&post_tag.tag_id]),
                    category: category_names[&category_id].clone(),
                    usages,
                })
                .collect()
        })
        .collect())
}

fn get_comments(conn: &mut PgConnection, ctx: &Context, posts: &[Post]) -> QueryResult<Vec<Vec<CommentInfo>>> {
    type CommentData = (Comment, i64, Option<(SmallString, AvatarStyle)>);
    let comments: Vec<CommentData> = Comment::belonging_to(posts)
        .inner_join(comment_statistics::table)
        .left_join(user::table)
        .select((Comment::as_select(), comment_statistics::score, (user::name, user::avatar_style).nullable()))
        .order(comment::creation_time)
        .load(conn)?;
    let comment_ids: Vec<i64> = comments.iter().map(|(comment, ..)| comment.id).collect();

    let client_scores: HashMap<i64, Score> = ctx
        .client
        .id
        .map(|user_id| {
            comment_score::table
                .select((comment_score::comment_id, comment_score::score))
                .filter(comment_score::comment_id.eq_any(comment_ids))
                .filter(comment_score::user_id.eq(user_id))
                .load(conn)
        })
        .transpose()?
        .unwrap_or_default()
        .into_iter()
        .collect();

    Ok(posts
        .iter()
        .zip(comments.grouped_by(posts))
        .map(|(post, comments_on_post)| {
            comments_on_post
                .into_iter()
                .map(|(comment, score, owner)| {
                    let id = comment.id;
                    CommentInfo {
                        version: Some(comment.last_edit_time),
                        id: Some(id),
                        post_id: Some(post.id),
                        user: Some(
                            owner.map(|(username, avatar_style)| MicroUser::new(&ctx.config, username, avatar_style)),
                        ),
                        text: Some(comment.text),
                        creation_time: Some(comment.creation_time),
                        last_edit_time: Some(comment.last_edit_time),
                        score: Some(score),
                        own_score: Some(client_scores.get(&id).copied().map(Rating::from).unwrap_or_default()),
                    }
                })
                .collect()
        })
        .collect())
}

fn get_relations(conn: &mut PgConnection, ctx: &Context, posts: &[Post]) -> QueryResult<Vec<Vec<MicroPost>>> {
    let mut related_posts = PostRelation::belonging_to(posts)
        .order(post_relation::child_id)
        .into_boxed();

    // Apply preference filters to post relations
    if let Some(hidden_posts) = ctx.preferences().hidden_posts(post_relation::child_id) {
        related_posts = related_posts.filter(not(exists(hidden_posts)));
    }

    Ok(related_posts
        .load::<PostRelation>(conn)?
        .grouped_by(posts)
        .into_iter()
        .map(|post_relations| {
            post_relations
                .into_iter()
                .map(|relation| MicroPost {
                    id: relation.child_id,
                    thumbnail_url: PostHash::new(&ctx.config, relation.child_id, None).thumbnail_url(),
                })
                .collect()
        })
        .collect())
}

fn get_pools(conn: &mut PgConnection, posts: &[Post]) -> QueryResult<Vec<Vec<MicroPool>>> {
    let pool_posts: Vec<(PoolPost, i64, i64)> = PoolPost::belonging_to(posts)
        .inner_join(pool::table.inner_join(pool_statistics::table))
        .select((PoolPost::as_select(), pool::category_id, pool_statistics::post_count))
        .order((pool::category_id, pool::id))
        .load(conn)?;
    let pool_ids: HashSet<i64> = pool_posts.iter().map(|(pool_post, ..)| pool_post.pool_id).collect();

    let pool_names: Vec<(i64, SmallString)> = pool_name::table
        .select((pool_name::pool_id, pool_name::name))
        .filter(pool_name::pool_id.eq_any(&pool_ids))
        .order((pool_name::pool_id, pool_name::order, pool_name::name))
        .load(conn)?;
    let names_map = resource::collect_names(pool_names);

    let category_names: HashMap<i64, SmallString> = pool_category::table
        .select((pool_category::id, pool_category::name))
        .load(conn)?
        .into_iter()
        .collect();
    let pool_descriptions: HashMap<i64, LargeString> = pool::table
        .select((pool::id, pool::description))
        .filter(pool::id.eq_any(pool_ids))
        .load(conn)?
        .into_iter()
        .collect();

    Ok(pool_posts
        .grouped_by(posts)
        .into_iter()
        .map(|pools_on_post| {
            pools_on_post
                .into_iter()
                .map(|(pool_post, category_id, post_count)| MicroPool {
                    id: pool_post.pool_id,
                    names: Arc::clone(&names_map[&pool_post.pool_id]),
                    category: category_names[&category_id].clone(),
                    description: pool_descriptions[&pool_post.pool_id].clone(),
                    post_count,
                })
                .collect()
        })
        .collect())
}

fn get_notes(conn: &mut PgConnection, posts: &[Post]) -> QueryResult<Vec<Vec<Note>>> {
    Ok(PostNote::belonging_to(posts)
        .order(post_note::id)
        .load(conn)?
        .grouped_by(posts)
        .into_iter()
        .map(|post_notes| post_notes.into_iter().map(Note::new).collect())
        .collect())
}

fn get_own_scores(conn: &mut PgConnection, client: Client, posts: &[Post]) -> QueryResult<Vec<Rating>> {
    if let Some(client_id) = client.id {
        PostScore::belonging_to(posts)
            .filter(post_score::user_id.eq(client_id))
            .load::<PostScore>(conn)
            .map(|client_scores| {
                resource::order_like(client_scores, posts, |score| score.post_id)
                    .into_iter()
                    .map(|client_score| client_score.map(|score| Rating::from(score.score)).unwrap_or_default())
                    .collect()
            })
    } else {
        Ok(vec![Rating::default(); posts.len()])
    }
}

fn get_own_favorites(conn: &mut PgConnection, client: Client, posts: &[Post]) -> QueryResult<Vec<bool>> {
    if let Some(client_id) = client.id {
        PostFavorite::belonging_to(posts)
            .filter(post_favorite::user_id.eq(client_id))
            .load::<PostFavorite>(conn)
            .map(|client_favorites| {
                resource::order_like(client_favorites, posts, |favorite| favorite.post_id)
                    .into_iter()
                    .map(|client_favorite| client_favorite.is_some())
                    .collect()
            })
    } else {
        Ok(vec![false; posts.len()])
    }
}

fn get_favorited_by(conn: &mut PgConnection, config: &Config, posts: &[Post]) -> QueryResult<Vec<Vec<MicroUser>>> {
    let users_who_favorited: Vec<(PostFavorite, SmallString, AvatarStyle)> = PostFavorite::belonging_to(posts)
        .inner_join(user::table)
        .select((PostFavorite::as_select(), user::name, user::avatar_style))
        .order(user::name)
        .load(conn)?;
    Ok(users_who_favorited
        .grouped_by(posts)
        .into_iter()
        .map(|user_favorites| {
            user_favorites
                .into_iter()
                .map(|(_, username, avatar_style)| MicroUser::new(config, username, avatar_style))
                .collect()
        })
        .collect())
}

#[doc(hidden)]
#[macro_export]
macro_rules! post_stats {
    ($conn:expr, $posts:expr, $column:expr, $return_type:ty) => {{
        let post_ids: Vec<_> = $posts.iter().map(Identifiable::id).copied().collect();
        $crate::schema::post_statistics::table
            .select(($crate::schema::post_statistics::post_id, $column))
            .filter($crate::schema::post_statistics::post_id.eq_any(&post_ids))
            .load($conn)
            .map(|post_stats| {
                resource::order_transformed_as(post_stats, &post_ids, |&(id, _)| id)
                    .into_iter()
                    .map(|(_, stat)| stat)
                    .collect::<Vec<$return_type>>()
            })
    }};
}
