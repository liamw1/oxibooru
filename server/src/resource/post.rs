use crate::auth::content;
use crate::model::comment::Comment;
use crate::model::enums::{AvatarStyle, DatabaseScore, MimeType, PostSafety, PostType, Score};
use crate::model::pool::{Pool, PoolPost};
use crate::model::post::{Post, PostFavorite, PostFeature, PostNote, PostRelation, PostScore, PostTag};
use crate::resource;
use crate::resource::comment::CommentInfo;
use crate::resource::pool::MicroPool;
use crate::resource::tag::MicroTag;
use crate::resource::user::MicroUser;
use crate::schema::{
    comment, comment_score, pool, pool_category, pool_name, pool_post, post, post_favorite, post_feature, post_note,
    post_relation, post_score, post_tag, tag, tag_category, tag_name, user,
};
use crate::util::DateTime;
use diesel::dsl::*;
use diesel::prelude::*;
use serde::Serialize;
use serde_with::skip_serializing_none;
use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use strum::{EnumString, EnumTable};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MicroPost {
    pub id: i32,
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

#[skip_serializing_none]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PostInfo {
    version: Option<DateTime>,
    id: Option<i32>,
    user: Option<Option<MicroUser>>,
    file_size: Option<i64>,
    canvas_width: Option<i32>,
    canvas_height: Option<i32>,
    safety: Option<PostSafety>,
    type_: Option<PostType>,
    mime_type: Option<MimeType>,
    checksum: Option<String>,
    #[serde(rename = "checksumMD5")]
    checksum_md5: Option<Option<String>>,
    flags: Option<Option<String>>,
    source: Option<Option<String>>,
    creation_time: Option<DateTime>,
    last_edit_time: Option<DateTime>,
    content_url: Option<String>,
    thumbnail_url: Option<String>,
    tags: Option<Vec<MicroTag>>,
    comments: Option<Vec<CommentInfo>>,
    relations: Option<Vec<MicroPost>>,
    pools: Option<Vec<MicroPool>>,
    notes: Option<Vec<PostNoteInfo>>,
    score: Option<i64>,
    own_score: Option<Score>,
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
    pub fn new(
        conn: &mut PgConnection,
        client: Option<i32>,
        post: Post,
        fields: &FieldTable<bool>,
    ) -> QueryResult<Self> {
        let mut post_info = Self::new_batch(conn, client, vec![post], fields)?;
        assert_eq!(post_info.len(), 1);
        Ok(post_info.pop().unwrap())
    }

    pub fn new_from_id(
        conn: &mut PgConnection,
        client: Option<i32>,
        post_id: i32,
        fields: &FieldTable<bool>,
    ) -> QueryResult<Self> {
        let mut post_info = Self::new_batch_from_ids(conn, client, vec![post_id], fields)?;
        assert_eq!(post_info.len(), 1);
        Ok(post_info.pop().unwrap())
    }

    fn new_batch(
        conn: &mut PgConnection,
        client: Option<i32>,
        posts: Vec<Post>,
        fields: &FieldTable<bool>,
    ) -> QueryResult<Vec<Self>> {
        let batch_size = posts.len();

        let mut owners = fields[Field::User]
            .then_some(get_post_owners(conn, &posts)?)
            .unwrap_or_default();
        resource::check_batch_results(owners.len(), batch_size);

        let mut content_urls = fields[Field::ContentUrl]
            .then_some(get_content_urls(&posts))
            .unwrap_or_default();
        resource::check_batch_results(content_urls.len(), batch_size);

        let mut thumbnail_urls = fields[Field::ThumbnailUrl]
            .then_some(get_thumbnail_urls(&posts))
            .unwrap_or_default();
        resource::check_batch_results(thumbnail_urls.len(), batch_size);

        let mut tags = fields[Field::Tags]
            .then_some(get_tags(conn, &posts)?)
            .unwrap_or_default();
        resource::check_batch_results(tags.len(), batch_size);

        let mut comments = fields[Field::Comments]
            .then_some(get_comments(conn, client, &posts)?)
            .unwrap_or_default();
        resource::check_batch_results(comments.len(), batch_size);

        let mut relations = fields[Field::Relations]
            .then_some(get_relations(conn, &posts)?)
            .unwrap_or_default();
        resource::check_batch_results(relations.len(), batch_size);

        let mut pools = fields[Field::Pools]
            .then_some(get_pools(conn, &posts)?)
            .unwrap_or_default();
        resource::check_batch_results(pools.len(), batch_size);

        let mut notes = fields[Field::Notes]
            .then_some(get_notes(conn, &posts)?)
            .unwrap_or_default();
        resource::check_batch_results(notes.len(), batch_size);

        let mut scores = fields[Field::Score]
            .then_some(get_scores(conn, &posts)?)
            .unwrap_or_default();
        resource::check_batch_results(scores.len(), batch_size);

        let mut client_scores = fields[Field::OwnScore]
            .then_some(get_client_scores(conn, client, &posts)?)
            .unwrap_or_default();
        resource::check_batch_results(client_scores.len(), batch_size);

        let mut client_favorites = fields[Field::OwnFavorite]
            .then_some(get_client_favorites(conn, client, &posts)?)
            .unwrap_or_default();
        resource::check_batch_results(client_favorites.len(), batch_size);

        let mut tag_counts = fields[Field::TagCount]
            .then_some(get_tag_counts(conn, &posts)?)
            .unwrap_or_default();
        resource::check_batch_results(tag_counts.len(), batch_size);

        let mut comment_counts = fields[Field::CommentCount]
            .then_some(get_comment_counts(conn, &posts)?)
            .unwrap_or_default();
        resource::check_batch_results(comment_counts.len(), batch_size);

        let mut relation_counts = fields[Field::RelationCount]
            .then_some(get_relation_counts(conn, &posts)?)
            .unwrap_or_default();
        resource::check_batch_results(relation_counts.len(), batch_size);

        let mut note_counts = fields[Field::NoteCount]
            .then_some(get_note_counts(conn, &posts)?)
            .unwrap_or_default();
        resource::check_batch_results(note_counts.len(), batch_size);

        let mut favorite_counts = fields[Field::FavoriteCount]
            .then_some(get_favorite_counts(conn, &posts)?)
            .unwrap_or_default();
        resource::check_batch_results(favorite_counts.len(), batch_size);

        let mut feature_counts = fields[Field::FeatureCount]
            .then_some(get_feature_counts(conn, &posts)?)
            .unwrap_or_default();
        resource::check_batch_results(feature_counts.len(), batch_size);

        let mut last_feature_times = fields[Field::LastFeatureTime]
            .then_some(get_last_feature_times(conn, &posts)?)
            .unwrap_or_default();
        resource::check_batch_results(last_feature_times.len(), batch_size);

        let mut users_who_favorited = fields[Field::FavoritedBy]
            .then_some(get_users_who_favorited(conn, &posts)?)
            .unwrap_or_default();
        resource::check_batch_results(users_who_favorited.len(), batch_size);

        let results = posts
            .into_iter()
            .rev()
            .map(|post| {
                Self {
                    version: fields[Field::Version].then_some(post.last_edit_time),
                    id: fields[Field::Id].then_some(post.id),
                    user: owners.pop(),
                    file_size: fields[Field::FileSize].then_some(post.file_size),
                    canvas_width: fields[Field::CanvasWidth].then_some(post.width),
                    canvas_height: fields[Field::CanvasHeight].then_some(post.height),
                    safety: fields[Field::Safety].then_some(post.safety),
                    type_: fields[Field::Type].then_some(post.type_),
                    mime_type: fields[Field::MimeType].then_some(post.mime_type),
                    checksum: fields[Field::Checksum].then_some(post.checksum),
                    checksum_md5: fields[Field::ChecksumMd5].then_some(post.checksum_md5),
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
                    has_custom_thumbnail: fields[Field::HasCustomThumbnail].then_some(false), // TODO
                    comments: comments.pop(),
                    pools: pools.pop(),
                }
            })
            .collect::<Vec<_>>();
        Ok(results.into_iter().rev().collect())
    }

    pub fn new_batch_from_ids(
        conn: &mut PgConnection,
        client: Option<i32>,
        post_ids: Vec<i32>,
        fields: &FieldTable<bool>,
    ) -> QueryResult<Vec<Self>> {
        let posts = get_posts(conn, &post_ids)?;
        Self::new_batch(conn, client, posts, fields)
    }
}

#[derive(Serialize)]
struct PostNoteInfo {
    polygon: Vec<u8>, // Probably not correct type, TODO
    text: String,
}

impl PostNoteInfo {
    pub fn new(note: PostNote) -> Self {
        PostNoteInfo {
            polygon: note.polygon,
            text: note.text,
        }
    }
}

fn get_posts(conn: &mut PgConnection, post_ids: &[i32]) -> QueryResult<Vec<Post>> {
    // We get posts here, but this query doesn't preserve order
    let posts = post::table.filter(post::id.eq_any(post_ids)).load(conn)?;
    Ok(resource::order_by(posts, post_ids))
}

fn get_post_owners(conn: &mut PgConnection, posts: &[Post]) -> QueryResult<Vec<Option<MicroUser>>> {
    let post_ids = posts.iter().map(|post| post.id).collect::<Vec<_>>();
    post::table
        .filter(post::id.eq_any(&post_ids))
        .inner_join(user::table)
        .select((post::id, user::name, user::avatar_style))
        .load::<(i32, String, AvatarStyle)>(conn)
        .map(|post_info| {
            resource::order_as(post_info, posts, |(id, ..)| *id)
                .into_iter()
                .map(|post_owner| post_owner.map(|(_, username, avatar_style)| MicroUser::new(username, avatar_style)))
                .collect()
        })
}

fn get_content_urls(posts: &[Post]) -> Vec<String> {
    posts
        .iter()
        .map(|post| content::post_content_url(post.id, post.mime_type))
        .collect()
}

fn get_thumbnail_urls(posts: &[Post]) -> Vec<String> {
    posts
        .iter()
        .map(|post| post.id)
        .map(content::post_thumbnail_url)
        .collect()
}

fn get_tags(conn: &mut PgConnection, posts: &[Post]) -> QueryResult<Vec<Vec<MicroTag>>> {
    let post_tags: Vec<(PostTag, i32, String)> = PostTag::belonging_to(posts)
        .inner_join(tag::table.inner_join(tag_name::table))
        .select((PostTag::as_select(), tag::category_id, tag_name::name))
        .order((tag_name::order, tag_name::name))
        .load(conn)?;
    let all_tag_ids: HashSet<i32> = post_tags.iter().map(|(post_tag, ..)| post_tag.tag_id).collect();

    let usages: HashMap<i32, i64> = post_tag::table
        .group_by(post_tag::tag_id)
        .select((post_tag::tag_id, count(post_tag::tag_id)))
        .filter(post_tag::tag_id.eq_any(all_tag_ids))
        .load(conn)?
        .into_iter()
        .collect();
    let category_names: HashMap<i32, String> = tag_category::table
        .select((tag_category::id, tag_category::name))
        .load(conn)?
        .into_iter()
        .collect();

    Ok(post_tags
        .grouped_by(posts)
        .into_iter()
        .map(|tags_on_post| {
            resource::collect_tag_data(tags_on_post, |post_tag| post_tag.tag_id)
                .into_iter()
                .map(|tag| MicroTag {
                    names: tag.names,
                    category: category_names[&tag.category_id].clone(),
                    usages: usages.get(&tag.id).copied().unwrap_or(0),
                })
                .collect()
        })
        .collect())
}

fn get_comments(conn: &mut PgConnection, client: Option<i32>, posts: &[Post]) -> QueryResult<Vec<Vec<CommentInfo>>> {
    let comments: Vec<(Comment, String, AvatarStyle)> = Comment::belonging_to(posts)
        .inner_join(user::table)
        .select((Comment::as_select(), user::name, user::avatar_style))
        .order(comment::creation_time)
        .load(conn)?;
    let comment_ids: Vec<i32> = comments.iter().map(|(comment, ..)| comment.id).collect();
    let scores: HashMap<i32, Option<i64>> = comment_score::table
        .group_by(comment_score::comment_id)
        .select((comment_score::comment_id, sum(comment_score::score)))
        .filter(comment_score::comment_id.eq_any(&comment_ids))
        .load(conn)?
        .into_iter()
        .collect();
    let client_scores: HashMap<i32, DatabaseScore> = client
        .map(|user_id| {
            comment_score::table
                .select((comment_score::comment_id, comment_score::score))
                .filter(comment_score::comment_id.eq_any(&comment_ids))
                .filter(comment_score::user_id.eq(user_id))
                .load(conn)
        })
        .transpose()?
        .unwrap_or_default()
        .into_iter()
        .collect();

    Ok(posts
        .iter()
        .zip(comments.grouped_by(posts).into_iter())
        .map(|(post, comments_on_post)| {
            comments_on_post
                .into_iter()
                .map(|(comment, username, avatar_style)| {
                    let id = comment.id;
                    CommentInfo {
                        version: comment.last_edit_time,
                        id,
                        post_id: post.id,
                        user: MicroUser::new(username, avatar_style),
                        text: comment.text,
                        creation_time: comment.creation_time,
                        last_edit_time: comment.last_edit_time,
                        score: scores.get(&id).copied().flatten().unwrap_or(0),
                        own_score: client_scores.get(&id).copied().map(Score::from).unwrap_or_default(),
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
                    thumbnail_url: content::post_thumbnail_url(relation.child_id),
                })
                .collect()
        })
        .collect())
}

fn get_pools(conn: &mut PgConnection, posts: &[Post]) -> QueryResult<Vec<Vec<MicroPool>>> {
    let pools: Vec<Pool> = PoolPost::belonging_to(posts)
        .inner_join(pool::table)
        .select(Pool::as_select())
        .distinct()
        .load(conn)?;
    let usages: HashMap<i32, i64> = PoolPost::belonging_to(&pools)
        .group_by(pool_post::pool_id)
        .select((pool_post::pool_id, count(pool_post::pool_id)))
        .load(conn)?
        .into_iter()
        .collect();
    let category_names: HashMap<i32, String> = pool_category::table
        .select((pool_category::id, pool_category::name))
        .load(conn)?
        .into_iter()
        .collect();

    let pool_posts = PoolPost::belonging_to(posts)
        .inner_join(pool::table.inner_join(pool_name::table))
        .select((PoolPost::as_select(), pool_name::name))
        .order(pool_name::order)
        .load(conn)?;
    let process_pool = |pool_info: (&Pool, Vec<(PoolPost, String)>)| -> Option<MicroPool> {
        let (pool, pool_names) = pool_info;
        (!pool_names.is_empty()).then_some({
            MicroPool {
                id: pool.id,
                names: pool_names.into_iter().map(|(_, pool_name)| pool_name).collect(),
                category: category_names[&pool.category_id].clone(),
                description: pool.description.clone(),
                post_count: usages.get(&pool.id).copied().unwrap_or(0),
            }
        })
    };
    Ok(pool_posts
        .grouped_by(posts)
        .into_iter()
        .map(|pools_on_post| {
            pools
                .iter()
                .zip(pools_on_post.grouped_by(&pools).into_iter())
                .filter_map(process_pool)
                .collect()
        })
        .collect())
}

fn get_notes(conn: &mut PgConnection, posts: &[Post]) -> QueryResult<Vec<Vec<PostNoteInfo>>> {
    Ok(PostNote::belonging_to(posts)
        .load(conn)?
        .grouped_by(posts)
        .into_iter()
        .map(|post_notes| post_notes.into_iter().map(PostNoteInfo::new).collect())
        .collect())
}

fn get_scores(conn: &mut PgConnection, posts: &[Post]) -> QueryResult<Vec<i64>> {
    PostScore::belonging_to(posts)
        .group_by(post_score::post_id)
        .select((post_score::post_id, sum(post_score::score)))
        .load(conn)
        .map(|post_scores| {
            resource::order_as(post_scores, posts, |(id, _)| *id)
                .into_iter()
                .map(|post_score| post_score.map(|(_, score)| score).flatten().unwrap_or(0))
                .collect()
        })
}

fn get_client_scores(conn: &mut PgConnection, client: Option<i32>, posts: &[Post]) -> QueryResult<Vec<Score>> {
    if let Some(client_id) = client {
        PostScore::belonging_to(posts)
            .filter(post_score::user_id.eq(client_id))
            .load::<PostScore>(conn)
            .map(|client_scores| {
                resource::order_as(client_scores, posts, |score| score.post_id)
                    .into_iter()
                    .map(|client_score| client_score.map(|score| Score::from(score.score)).unwrap_or_default())
                    .collect()
            })
    } else {
        Ok(vec![Score::default(); posts.len()])
    }
}

fn get_client_favorites(conn: &mut PgConnection, client: Option<i32>, posts: &[Post]) -> QueryResult<Vec<bool>> {
    if let Some(client_id) = client {
        PostFavorite::belonging_to(posts)
            .filter(post_favorite::user_id.eq(client_id))
            .load::<PostFavorite>(conn)
            .map(|client_favorites| {
                resource::order_as(client_favorites, posts, |favorite| favorite.post_id)
                    .into_iter()
                    .map(|client_favorite| client_favorite.is_some())
                    .collect()
            })
    } else {
        Ok(vec![false; posts.len()])
    }
}

fn get_tag_counts(conn: &mut PgConnection, posts: &[Post]) -> QueryResult<Vec<i64>> {
    PostTag::belonging_to(posts)
        .group_by(post_tag::post_id)
        .select((post_tag::post_id, count(post_tag::tag_id)))
        .load(conn)
        .map(|tag_counts| {
            resource::order_as(tag_counts, posts, |(id, _)| *id)
                .into_iter()
                .map(|tag_count| tag_count.map(|(_, count)| count).unwrap_or(0))
                .collect()
        })
}

fn get_comment_counts(conn: &mut PgConnection, posts: &[Post]) -> QueryResult<Vec<i64>> {
    Comment::belonging_to(posts)
        .group_by(comment::post_id)
        .select((comment::post_id, count(comment::post_id)))
        .load(conn)
        .map(|comment_counts| {
            resource::order_as(comment_counts, posts, |(id, _)| *id)
                .into_iter()
                .map(|comment_count| comment_count.map(|(_, count)| count).unwrap_or(0))
                .collect()
        })
}

fn get_relation_counts(conn: &mut PgConnection, posts: &[Post]) -> QueryResult<Vec<i64>> {
    PostRelation::belonging_to(posts)
        .group_by(post_relation::parent_id)
        .select((post_relation::parent_id, count(post_relation::child_id)))
        .load(conn)
        .map(|relation_counts| {
            resource::order_as(relation_counts, posts, |(id, _)| *id)
                .into_iter()
                .map(|relation_count| relation_count.map(|(_, count)| count).unwrap_or(0))
                .collect()
        })
}

fn get_note_counts(conn: &mut PgConnection, posts: &[Post]) -> QueryResult<Vec<i64>> {
    PostNote::belonging_to(posts)
        .group_by(post_note::post_id)
        .select((post_note::post_id, count(post_note::id)))
        .load(conn)
        .map(|note_counts| {
            resource::order_as(note_counts, posts, |(id, _)| *id)
                .into_iter()
                .map(|note_count| note_count.map(|(_, count)| count).unwrap_or(0))
                .collect()
        })
}

fn get_favorite_counts(conn: &mut PgConnection, posts: &[Post]) -> QueryResult<Vec<i64>> {
    PostFavorite::belonging_to(posts)
        .group_by(post_favorite::post_id)
        .select((post_favorite::post_id, count(post_favorite::user_id)))
        .load(conn)
        .map(|favorite_counts| {
            resource::order_as(favorite_counts, posts, |(id, _)| *id)
                .into_iter()
                .map(|favorite_count| favorite_count.map(|(_, count)| count).unwrap_or(0))
                .collect()
        })
}

fn get_feature_counts(conn: &mut PgConnection, posts: &[Post]) -> QueryResult<Vec<i64>> {
    PostFeature::belonging_to(posts)
        .group_by(post_feature::post_id)
        .select((post_feature::post_id, count(post_feature::id)))
        .load(conn)
        .map(|feature_counts| {
            resource::order_as(feature_counts, posts, |(id, _)| *id)
                .into_iter()
                .map(|feature_count| feature_count.map(|(_, count)| count).unwrap_or(0))
                .collect()
        })
}

fn get_last_feature_times(conn: &mut PgConnection, posts: &[Post]) -> QueryResult<Vec<Option<DateTime>>> {
    PostFeature::belonging_to(posts)
        .group_by(post_feature::post_id)
        .select((post_feature::post_id, max(post_feature::time)))
        .load(conn)
        .map(|last_feature_times| {
            resource::order_as(last_feature_times, posts, |(id, _)| *id)
                .into_iter()
                .map(|last_feature_time| last_feature_time.map(|(_, time)| time).flatten())
                .collect()
        })
}

fn get_users_who_favorited(conn: &mut PgConnection, posts: &[Post]) -> QueryResult<Vec<Vec<MicroUser>>> {
    let users_who_favorited: Vec<(i32, String, AvatarStyle)> = PostFavorite::belonging_to(posts)
        .inner_join(user::table)
        .select((post_favorite::post_id, user::name, user::avatar_style))
        .load(conn)?;

    let mut users_grouped_by_posts: Vec<Vec<(String, AvatarStyle)>> =
        std::iter::repeat_with(|| vec![]).take(posts.len()).collect();
    for (post_id, username, avatar_style) in users_who_favorited.into_iter() {
        let index = posts.iter().position(|post| post.id == post_id).unwrap();
        users_grouped_by_posts[index].push((username, avatar_style));
    }

    Ok(posts
        .iter()
        .zip(users_grouped_by_posts.into_iter())
        .map(|(_, users)| {
            users
                .into_iter()
                .map(|(username, avatar_style)| MicroUser::new(username, avatar_style))
                .collect()
        })
        .collect())
}
