use crate::auth::header::Client;
use crate::content::hash::PostHash;
use crate::get_post_stats;
use crate::model::comment::Comment;
use crate::model::enums::{AvatarStyle, MimeType, PostFlags, PostSafety, PostType, Rating, Score};
use crate::model::pool::PoolPost;
use crate::model::post::{NewPostNote, Post, PostFavorite, PostNote, PostRelation, PostScore, PostTag};
use crate::model::tag::TagName;
use crate::resource::comment::CommentInfo;
use crate::resource::pool::MicroPool;
use crate::resource::tag::MicroTag;
use crate::resource::user::MicroUser;
use crate::resource::{self, BoolFill};
use crate::schema::{
    comment, comment_score, comment_statistics, pool, pool_category, pool_name, pool_statistics, post, post_favorite,
    post_relation, post_score, post_statistics, tag, tag_category, tag_name, tag_statistics, user,
};
use crate::time::DateTime;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use serde_with::skip_serializing_none;
use std::collections::{HashMap, HashSet};
use strum::{EnumString, EnumTable};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Note {
    polygon: Vec<[f32; 2]>,
    text: String,
}

impl Note {
    pub fn new(note: PostNote) -> Self {
        let polygon = note
            .polygon
            .chunks_exact(2)
            .map(|vertex| [vertex[0].unwrap(), vertex[1].unwrap()])
            .collect();
        Self {
            polygon,
            text: note.text,
        }
    }

    pub fn to_new_post_note(&self, post_id: i64) -> NewPostNote {
        NewPostNote {
            post_id,
            polygon: self.polygon.as_flattened(),
            text: &self.text,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MicroPost {
    pub id: i64,
    pub thumbnail_url: String,
}

#[derive(Clone, Copy, EnumString, EnumTable)]
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

impl BoolFill for FieldTable<bool> {
    fn filled(val: bool) -> Self {
        Self::filled(val)
    }
}

#[skip_serializing_none]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PostInfo {
    version: Option<DateTime>,
    id: Option<i64>,
    user: Option<Option<MicroUser>>,
    file_size: Option<i64>,
    canvas_width: Option<i32>,
    canvas_height: Option<i32>,
    safety: Option<PostSafety>,
    type_: Option<PostType>,
    mime_type: Option<MimeType>,
    checksum: Option<String>,
    #[serde(rename = "checksumMD5")]
    checksum_md5: Option<String>,
    flags: Option<PostFlags>,
    source: Option<String>,
    creation_time: Option<DateTime>,
    last_edit_time: Option<DateTime>,
    content_url: Option<String>,
    thumbnail_url: Option<String>,
    tags: Option<Vec<MicroTag>>,
    comments: Option<Vec<CommentInfo>>,
    relations: Option<Vec<MicroPost>>,
    pools: Option<Vec<MicroPool>>,
    notes: Option<Vec<Note>>,
    score: Option<i64>,
    own_score: Option<Rating>,
    own_favorite: Option<bool>,
    tag_count: Option<i64>,
    comment_count: Option<i64>,
    relation_count: Option<i64>,
    note_count: Option<i64>,
    favorite_count: Option<i64>,
    feature_count: Option<i64>,
    last_feature_time: Option<Option<DateTime>>,
    favorited_by: Option<Vec<MicroUser>>,
    has_custom_thumbnail: Option<bool>,
}

impl PostInfo {
    pub fn new(conn: &mut PgConnection, client: Client, post: Post, fields: &FieldTable<bool>) -> QueryResult<Self> {
        let mut post_info = Self::new_batch(conn, client, vec![post], fields)?;
        assert_eq!(post_info.len(), 1);
        Ok(post_info.pop().unwrap())
    }

    pub fn new_from_id(
        conn: &mut PgConnection,
        client: Client,
        post_id: i64,
        fields: &FieldTable<bool>,
    ) -> QueryResult<Self> {
        let mut post_info = Self::new_batch_from_ids(conn, client, vec![post_id], fields)?;
        assert_eq!(post_info.len(), 1);
        Ok(post_info.pop().unwrap())
    }

    pub fn new_batch(
        conn: &mut PgConnection,
        client: Client,
        posts: Vec<Post>,
        fields: &FieldTable<bool>,
    ) -> QueryResult<Vec<Self>> {
        let batch_size = posts.len();

        let mut owners = fields[Field::User]
            .then(|| get_owners(conn, &posts))
            .transpose()?
            .unwrap_or_default();
        resource::check_batch_results(owners.len(), batch_size);

        let mut content_urls = fields[Field::ContentUrl]
            .then(|| get_content_urls(&posts))
            .unwrap_or_default();
        resource::check_batch_results(content_urls.len(), batch_size);

        let mut thumbnail_urls = fields[Field::ThumbnailUrl]
            .then(|| get_thumbnail_urls(&posts))
            .unwrap_or_default();
        resource::check_batch_results(thumbnail_urls.len(), batch_size);

        let mut tags = fields[Field::Tags]
            .then(|| get_tags(conn, &posts))
            .transpose()?
            .unwrap_or_default();
        resource::check_batch_results(tags.len(), batch_size);

        let mut comments = fields[Field::Comments]
            .then(|| get_comments(conn, client, &posts))
            .transpose()?
            .unwrap_or_default();
        resource::check_batch_results(comments.len(), batch_size);

        let mut relations = fields[Field::Relations]
            .then(|| get_relations(conn, &posts))
            .transpose()?
            .unwrap_or_default();
        resource::check_batch_results(relations.len(), batch_size);

        let mut pools = fields[Field::Pools]
            .then(|| get_pools(conn, &posts))
            .transpose()?
            .unwrap_or_default();
        resource::check_batch_results(pools.len(), batch_size);

        let mut notes = fields[Field::Notes]
            .then(|| get_notes(conn, &posts))
            .transpose()?
            .unwrap_or_default();
        resource::check_batch_results(notes.len(), batch_size);

        let mut scores = fields[Field::Score]
            .then(|| get_post_stats!(conn, &posts, post_statistics::score, i64))
            .transpose()?
            .unwrap_or_default();
        resource::check_batch_results(scores.len(), batch_size);

        let mut client_scores = fields[Field::OwnScore]
            .then(|| get_client_scores(conn, client, &posts))
            .transpose()?
            .unwrap_or_default();
        resource::check_batch_results(client_scores.len(), batch_size);

        let mut client_favorites = fields[Field::OwnFavorite]
            .then(|| get_client_favorites(conn, client, &posts))
            .transpose()?
            .unwrap_or_default();
        resource::check_batch_results(client_favorites.len(), batch_size);

        let mut tag_counts = fields[Field::TagCount]
            .then(|| get_post_stats!(conn, &posts, post_statistics::tag_count, i64))
            .transpose()?
            .unwrap_or_default();
        resource::check_batch_results(tag_counts.len(), batch_size);

        let mut comment_counts = fields[Field::CommentCount]
            .then(|| get_post_stats!(conn, &posts, post_statistics::comment_count, i64))
            .transpose()?
            .unwrap_or_default();
        resource::check_batch_results(comment_counts.len(), batch_size);

        let mut relation_counts = fields[Field::RelationCount]
            .then(|| get_post_stats!(conn, &posts, post_statistics::relation_count, i64))
            .transpose()?
            .unwrap_or_default();
        resource::check_batch_results(relation_counts.len(), batch_size);

        let mut note_counts = fields[Field::NoteCount]
            .then(|| get_post_stats!(conn, &posts, post_statistics::note_count, i64))
            .transpose()?
            .unwrap_or_default();
        resource::check_batch_results(note_counts.len(), batch_size);

        let mut favorite_counts = fields[Field::FavoriteCount]
            .then(|| get_post_stats!(conn, &posts, post_statistics::favorite_count, i64))
            .transpose()?
            .unwrap_or_default();
        resource::check_batch_results(favorite_counts.len(), batch_size);

        let mut feature_counts = fields[Field::FeatureCount]
            .then(|| get_post_stats!(conn, &posts, post_statistics::feature_count, i64))
            .transpose()?
            .unwrap_or_default();
        resource::check_batch_results(feature_counts.len(), batch_size);

        let mut last_feature_times = fields[Field::LastFeatureTime]
            .then(|| get_post_stats!(conn, &posts, post_statistics::last_feature_time, Option<DateTime>))
            .transpose()?
            .unwrap_or_default();
        resource::check_batch_results(last_feature_times.len(), batch_size);

        let mut users_who_favorited = fields[Field::FavoritedBy]
            .then(|| get_users_who_favorited(conn, &posts))
            .transpose()?
            .unwrap_or_default();
        resource::check_batch_results(users_who_favorited.len(), batch_size);

        let results = posts
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
                creation_time: fields[Field::CreationTime].then_some(post.creation_time),
                last_edit_time: fields[Field::LastEditTime].then_some(post.last_edit_time),
                content_url: content_urls.pop(),
                thumbnail_url: thumbnail_urls.pop(),
                tags: tags.pop(),
                relations: relations.pop(),
                notes: notes.pop(),
                score: scores.pop(),
                own_score: client_scores.pop(),
                own_favorite: client_favorites.pop(),
                tag_count: tag_counts.pop(),
                favorite_count: favorite_counts.pop(),
                comment_count: comment_counts.pop(),
                note_count: note_counts.pop(),
                feature_count: feature_counts.pop(),
                relation_count: relation_counts.pop(),
                last_feature_time: last_feature_times.pop(),
                favorited_by: users_who_favorited.pop(),
                comments: comments.pop(),
                pools: pools.pop(),
                has_custom_thumbnail: fields[Field::HasCustomThumbnail]
                    .then(|| PostHash::new(post.id).custom_thumbnail_path().exists()),
            })
            .collect::<Vec<_>>();
        Ok(results.into_iter().rev().collect())
    }

    pub fn new_batch_from_ids(
        conn: &mut PgConnection,
        client: Client,
        post_ids: Vec<i64>,
        fields: &FieldTable<bool>,
    ) -> QueryResult<Vec<Self>> {
        let unordered_posts = post::table.filter(post::id.eq_any(&post_ids)).load(conn)?;
        let posts = resource::order_as(unordered_posts, &post_ids);
        Self::new_batch(conn, client, posts, fields)
    }
}

fn get_owners(conn: &mut PgConnection, posts: &[Post]) -> QueryResult<Vec<Option<MicroUser>>> {
    let post_ids: Vec<_> = posts.iter().map(Identifiable::id).copied().collect();
    post::table
        .filter(post::id.eq_any(&post_ids))
        .inner_join(user::table)
        .select((post::id, user::name, user::avatar_style))
        .load::<(i64, String, AvatarStyle)>(conn)
        .map(|post_info| {
            resource::order_like(post_info, posts, |&(id, ..)| id)
                .into_iter()
                .map(|post_owner| post_owner.map(|(_, username, avatar_style)| MicroUser::new(username, avatar_style)))
                .collect()
        })
}

fn get_content_urls(posts: &[Post]) -> Vec<String> {
    posts
        .iter()
        .map(|post| PostHash::new(post.id).content_url(post.mime_type))
        .collect()
}

fn get_thumbnail_urls(posts: &[Post]) -> Vec<String> {
    posts
        .iter()
        .map(|post| PostHash::new(post.id).thumbnail_url())
        .collect()
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

    let tag_names: Vec<(i64, String)> = tag_name::table
        .select((tag_name::tag_id, tag_name::name))
        .filter(tag_name::tag_id.eq_any(tag_ids))
        .order((tag_name::tag_id, tag_name::order))
        .load(conn)?;
    let category_names: HashMap<i64, String> = tag_category::table
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
                .map(|(post_tag, category_id, usages)| {
                    let names = tag_names
                        .iter()
                        .skip_while(|&&(tag_id, _)| tag_id != post_tag.tag_id)
                        .take_while(|&&(tag_id, _)| tag_id == post_tag.tag_id)
                        .map(|(_, name)| name)
                        .cloned()
                        .collect();
                    MicroTag {
                        names,
                        category: category_names[&category_id].clone(),
                        usages,
                    }
                })
                .collect()
        })
        .collect())
}

fn get_comments(conn: &mut PgConnection, client: Client, posts: &[Post]) -> QueryResult<Vec<Vec<CommentInfo>>> {
    type CommentData = (Comment, i64, Option<(String, AvatarStyle)>);
    let comments: Vec<CommentData> = Comment::belonging_to(posts)
        .inner_join(comment_statistics::table)
        .left_join(user::table)
        .select((Comment::as_select(), comment_statistics::score, (user::name, user::avatar_style).nullable()))
        .order(comment::creation_time)
        .load(conn)?;
    let comment_ids: Vec<i64> = comments.iter().map(|(comment, ..)| comment.id).collect();

    let client_scores: HashMap<i64, Score> = client
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
                        user: Some(owner.map(|(username, avatar_style)| MicroUser::new(username, avatar_style))),
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

fn get_relations(conn: &mut PgConnection, posts: &[Post]) -> QueryResult<Vec<Vec<MicroPost>>> {
    let related_posts: Vec<PostRelation> = PostRelation::belonging_to(posts)
        .order(post_relation::child_id)
        .load(conn)?;
    Ok(related_posts
        .grouped_by(posts)
        .into_iter()
        .map(|post_relations| {
            post_relations
                .into_iter()
                .map(|relation| MicroPost {
                    id: relation.child_id,
                    thumbnail_url: PostHash::new(relation.child_id).thumbnail_url(),
                })
                .collect()
        })
        .collect())
}

fn get_pools(conn: &mut PgConnection, posts: &[Post]) -> QueryResult<Vec<Vec<MicroPool>>> {
    let pool_posts: Vec<(PoolPost, i64, String, i64)> = PoolPost::belonging_to(posts)
        .inner_join(pool::table.inner_join(pool_statistics::table))
        .select((PoolPost::as_select(), pool::category_id, pool::description, pool_statistics::post_count))
        .order((pool::category_id, pool::id))
        .load(conn)?;
    let pool_ids: HashSet<i64> = pool_posts.iter().map(|(pool_post, ..)| pool_post.pool_id).collect();

    let pool_names: Vec<(i64, String)> = pool_name::table
        .select((pool_name::pool_id, pool_name::name))
        .filter(pool_name::pool_id.eq_any(&pool_ids))
        .order((pool_name::pool_id, pool_name::order, pool_name::name))
        .load(conn)?;
    let category_names: HashMap<i64, String> = pool_category::table
        .select((pool_category::id, pool_category::name))
        .load(conn)?
        .into_iter()
        .collect();

    Ok(pool_posts
        .grouped_by(posts)
        .into_iter()
        .map(|pools_on_post| {
            pools_on_post
                .into_iter()
                .map(|(pool_post, category_id, description, post_count)| {
                    let names = pool_names
                        .iter()
                        .skip_while(|&&(pool_id, _)| pool_id != pool_post.pool_id)
                        .take_while(|&&(pool_id, _)| pool_id == pool_post.pool_id)
                        .map(|(_, name)| name)
                        .cloned()
                        .collect();
                    MicroPool {
                        id: pool_post.pool_id,
                        names,
                        category: category_names[&category_id].clone(),
                        description,
                        post_count,
                    }
                })
                .collect()
        })
        .collect())
}

fn get_notes(conn: &mut PgConnection, posts: &[Post]) -> QueryResult<Vec<Vec<Note>>> {
    Ok(PostNote::belonging_to(posts)
        .load(conn)?
        .grouped_by(posts)
        .into_iter()
        .map(|post_notes| post_notes.into_iter().map(Note::new).collect())
        .collect())
}

fn get_client_scores(conn: &mut PgConnection, client: Client, posts: &[Post]) -> QueryResult<Vec<Rating>> {
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

fn get_client_favorites(conn: &mut PgConnection, client: Client, posts: &[Post]) -> QueryResult<Vec<bool>> {
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

fn get_users_who_favorited(conn: &mut PgConnection, posts: &[Post]) -> QueryResult<Vec<Vec<MicroUser>>> {
    let users_who_favorited: Vec<(i64, String, AvatarStyle)> = PostFavorite::belonging_to(posts)
        .inner_join(user::table)
        .select((post_favorite::post_id, user::name, user::avatar_style))
        .order_by(user::name)
        .load(conn)?;

    let mut users_grouped_by_posts: Vec<Vec<(String, AvatarStyle)>> =
        std::iter::repeat_with(Vec::new).take(posts.len()).collect();
    for (post_id, username, avatar_style) in users_who_favorited.into_iter() {
        let index = posts.iter().position(|post| post.id == post_id).unwrap();
        users_grouped_by_posts[index].push((username, avatar_style));
    }

    Ok(posts
        .iter()
        .zip(users_grouped_by_posts)
        .map(|(_, users)| {
            users
                .into_iter()
                .map(|(username, avatar_style)| MicroUser::new(username, avatar_style))
                .collect()
        })
        .collect())
}

#[doc(hidden)]
#[macro_export]
macro_rules! get_post_stats {
    ($conn:expr, $posts:expr, $column:expr, $return_type:ty) => {{
        let post_ids: Vec<_> = $posts.iter().map(Identifiable::id).copied().collect();
        post_statistics::table
            .select((post_statistics::post_id, $column))
            .filter(post_statistics::post_id.eq_any(&post_ids))
            .load($conn)
            .map(|post_stats| {
                resource::order_transformed_as(post_stats, &post_ids, |&(id, _)| id)
                    .into_iter()
                    .map(|(_, stat)| stat)
                    .collect::<Vec<$return_type>>()
            })
    }};
}
