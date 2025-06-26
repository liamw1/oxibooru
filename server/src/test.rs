use crate::api::ApiResult;
use crate::auth::header;
use crate::db::{ConnectionPool, ConnectionResult};
use crate::model::comment::{NewComment, NewCommentScore};
use crate::model::enums::{AvatarStyle, MimeType, PostFlag, PostFlags, Score, UserRank};
use crate::model::enums::{PostSafety, PostType};
use crate::model::pool::{NewPool, NewPoolCategory, NewPoolName, PoolPost};
use crate::model::post::{NewPost, NewPostFeature, NewPostNote, PostFavorite, PostRelation, PostScore, PostTag};
use crate::model::tag::{NewTag, NewTagCategory, NewTagName, TagImplication, TagSuggestion};
use crate::model::user::{NewUser, NewUserToken};
use crate::schema::{
    comment, comment_score, pool, pool_category, pool_category_statistics, pool_name, pool_post, pool_statistics, post,
    post_favorite, post_feature, post_note, post_relation, post_score, post_statistics, post_tag, tag, tag_category,
    tag_category_statistics, tag_implication, tag_name, tag_statistics, tag_suggestion, user, user_token,
};
use crate::time::DateTime;
use crate::{api, db};
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool};
use std::error::Error;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;
use uuid::Uuid;

pub const TEST_PASSWORD: &str = "test_password";
pub const TEST_SALT: &str = "test_salt";
pub const TEST_HASH: &str = "$argon2id$v=19$m=19456,t=2,p=1$dGVzdF9zYWx0$voqGcDZhS6JWiMJy9q12zBgrC6OTBKa9dL8k0O8gD4M";
pub const TEST_TOKEN: Uuid = uuid::uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8");

pub fn get_connection() -> ConnectionResult {
    let mut lock = CONNECTION_POOL.lock().unwrap();
    match lock.as_mut() {
        Some(pool) => pool.get(),
        None => {
            let pool = recreate_database().unwrap();
            let conn = pool.get();
            *lock = Some(pool);
            conn
        }
    }
}

/// Resets the test database. Useful after operations that are hard to reverse perfectly, like merging.
pub fn reset_database() {
    let mut lock = CONNECTION_POOL.lock().unwrap();
    *lock = None;
}

/// Returns path to a test image.
pub fn image_path(relative_path: &str) -> PathBuf {
    asset_path("images", relative_path)
}

/// Returns path to an expected test request reply.
pub fn reply_path(relative_path: &str) -> PathBuf {
    asset_path("reply", relative_path)
}

/// Returns path to a test request body.
pub fn body_path(relative_path: &str) -> PathBuf {
    asset_path("body", relative_path)
}

/// Verifies that a given `query` matches the contents of a `reply_filepath`.
/// `query` must be of the form `METHOD path` (e.g. `GET /post/1`).
pub async fn verify_query(query: &str, relative_path: &str) -> ApiResult<()> {
    verify_query_with_user("administrator", query, relative_path).await
}

pub async fn verify_query_with_user(user: &str, query: &str, relative_path: &str) -> ApiResult<()> {
    let filter = api::routes();
    let (method, path) = query.split_once(' ').unwrap();
    let path = path.replace(' ', "%20"); // Percent-encode all spaces
    let credentials = header::credentials_for(user, TEST_PASSWORD);
    let basic_access_authentication = format!("Basic {credentials}");

    let request = warp::test::request()
        .header("authorization", basic_access_authentication)
        .method(method)
        .path(&path);

    // Optionally specify a body
    let body_path = body_path(relative_path);
    let reply = match body_path.try_exists()? {
        true => {
            let body = std::fs::read_to_string(body_path)?;
            request.body(body).reply(&filter).await
        }
        false => request.reply(&filter).await,
    };
    let actual_body = std::str::from_utf8(reply.body())?;

    let file_contents = std::fs::read_to_string(reply_path(relative_path))?;
    let expected_body: String = file_contents.split_whitespace().collect();
    let actual_body: String = actual_body.split_whitespace().collect();

    assert_eq!(reply.status(), 200);
    assert_eq!(actual_body, expected_body);
    Ok(())
}

const DATABASE_NAME: &str = "__test";

const USERS: &[NewUser] = &[
    new_user("restricted_user", None, UserRank::Restricted),
    new_user("regular_user", Some("email@domain.com"), UserRank::Regular),
    new_user("power_user", Some("example&hotmail.com"), UserRank::Power),
    new_user("moderator", None, UserRank::Moderator),
    new_user("administrator", None, UserRank::Administrator),
];

const POOL_CATEGORY_NAMES: &[&str] = &["Setting", "Style"];
const DEFAULT_POOLS: &[&[&str]] = &[&["favs"]];
const SETTINGS_POOLS: &[&[&str]] = &[&["fantasy"], &["steampunk"], &["cyberpunk"]];
const STYLES_POOLS: &[&[&str]] = &[&["abstract"], &["realistic"]];
const POOL_GROUPS: &[&[&[&str]]] = &[DEFAULT_POOLS, SETTINGS_POOLS, STYLES_POOLS];

const TAG_CATEGORY_NAMES: &[&str] = &["Artist", "Source", "Character", "Surroundings", "Meta"];
const DEFAULT_TAGS: &[&[&str]] = &[&["tagme", "tag_me"]];
const ARTIST_TAGS: &[&[&str]] = &[&["shakespeare"], &["george_lucas"], &["hidetaka_miyazaki"]];
const SOURCE_TAGS: &[&[&str]] = &[&["classic_literature"], &["star_wars"], &["sekiro"]];
const CHARACTER_TAGS: &[&[&str]] = &[
    &["claudius"],
    &["laertes"],
    &["ophelia"],
    &["luke_skywalker"],
    &["darth_vader", "annakin_skywalker"],
    &["princess_leia"],
    &["admiral_ackbar"],
    &["isshin_ashina"],
    &["kuro"],
    &["black_hat_badger"],
    &["sekiro_(sekiro)"],
];
const SURROUNDINGS_TAGS: &[&[&str]] = &[
    &["plant", "foliage"],
    &["tree"],
    &["forest", "woods"],
    &["rock", "stone"],
    &["water", "agua"],
    &["river", "stream", "creek"],
    &["sand"],
    &["desert"],
    &["night"],
    &["sky"],
    &["night_sky"],
];
const META_TAGS: &[&[&str]] = &[&["high_resolution", "high_res"], &["16:9_aspect_ratio"]];
const TAG_GROUPS: &[&[&[&str]]] = &[
    DEFAULT_TAGS,
    ARTIST_TAGS,
    SOURCE_TAGS,
    CHARACTER_TAGS,
    SURROUNDINGS_TAGS,
    META_TAGS,
];

const TAG_IMPLICATIONS: &[(&str, &str)] = &[
    ("tree", "plant"),
    ("forest", "plant"),
    ("forest", "tree"),
    ("river", "water"),
    ("desert", "sand"),
    ("night_sky", "night"),
    ("night_sky", "sky"),
];
const TAG_SUGGESTIONS: &[(&str, &str)] = &[("river", "sand"), ("river", "plant")];

const POST_1_TAGS: &[&str] = &[
    "shakespeare",
    "classic_literature",
    "claudius",
    "laertes",
    "plant",
    "rock",
];
const POST_2_TAGS: &[&str] = &[
    "george_lucas",
    "star_wars",
    "luke_skywalker",
    "darth_vader",
    "princess_leia",
    "admiral_ackbar",
    "forest",
    "tree",
    "plant",
    "rock",
    "river",
    "water",
    "16:9_aspect_ratio",
];
const POST_3_TAGS: &[&str] = &["high_resolution", "tagme"];
const POST_4_TAGS: &[&str] = &[];
const POST_5_TAGS: &[&str] = &[
    "hidetaka_miyazaki",
    "sekiro",
    "isshin_ashina",
    "black_hat_badger",
    "sekiro_(sekiro)",
    "night_sky",
    "night",
    "sky",
    "16:9_aspect_ratio",
];
const POST_TAGS: &[&[&str]] = &[POST_1_TAGS, POST_2_TAGS, POST_3_TAGS, POST_4_TAGS, POST_5_TAGS];

const POOL_POSTS: &[(&str, i64)] = &[
    ("fantasy", 1),
    ("fantasy", 2),
    ("cyberpunk", 2),
    ("abstract", 4),
    ("fantasy", 5),
    ("realistic", 5),
];

const MB: i64 = 1024_i64.pow(2);
const GB: i64 = 1024_i64.pow(3);
const POSTS: &[NewPost] = &[
    NewPost {
        user_id: Some(1),
        file_size: 1 * MB,
        width: 1000,
        height: 2000,
        safety: PostSafety::Safe,
        type_: PostType::Image,
        mime_type: MimeType::Jpeg,
        checksum: b"01",
        checksum_md5: b"01",
        flags: PostFlags::new(),
        source: "My hard drive",
    },
    NewPost {
        user_id: Some(2),
        file_size: 5 * MB,
        width: 1980,
        height: 1080,
        safety: PostSafety::Sketchy,
        type_: PostType::Animation,
        mime_type: MimeType::Gif,
        checksum: b"02",
        checksum_md5: b"02",
        flags: PostFlags::new(),
        source: "",
    },
    NewPost {
        user_id: Some(2),
        file_size: 92 * MB,
        width: 11146,
        height: 7479,
        safety: PostSafety::Safe,
        type_: PostType::Image,
        mime_type: MimeType::Png,
        checksum: b"03",
        checksum_md5: b"03",
        flags: PostFlags::new(),
        source: "Wikipedia",
    },
    NewPost {
        user_id: Some(2),
        file_size: 1,
        width: 1,
        height: 1,
        safety: PostSafety::Safe,
        type_: PostType::Image,
        mime_type: MimeType::Bmp,
        checksum: b"04",
        checksum_md5: b"04",
        flags: PostFlags::new(),
        source: "",
    },
    NewPost {
        user_id: None,
        file_size: 100 * GB,
        width: 1980,
        height: 1080,
        safety: PostSafety::Unsafe,
        type_: PostType::Video,
        mime_type: MimeType::Mp4,
        checksum: b"05",
        checksum_md5: b"05",
        flags: PostFlags::new_with(PostFlag::Sound),
        source: "???",
    },
];

const POST_RELATIONS: &[(i64, i64)] = &[(1, 2), (1, 3), (4, 5)];
const POST_FAVORITES: &[(i64, i64)] = &[(1, 1), (2, 2), (2, 3), (2, 4), (5, 5)];
const POST_FEATURES: &[(i64, i64)] = &[(5, 5), (4, 4), (3, 1), (3, 3), (3, 1)];
const POST_SCORES: &[(i64, i64, Score)] = &[
    (1, 5, Score::Dislike),
    (2, 1, Score::Like),
    (2, 2, Score::Like),
    (2, 3, Score::Like),
    (4, 4, Score::Like),
    (5, 4, Score::Dislike),
    (5, 5, Score::Like),
];

const COMMENTS: &[(Option<i64>, i64, &str)] = &[
    (Some(2), 1, "Cool post!"),
    (Some(5), 1, "how did you post this"),
    (Some(2), 4, "I don't think this uploaded correctly"),
    (None, 5, "Lorem ipsum dolor sit amet, consectetur adipiscing elit"),
];

const COMMENT_SCORES: &[(i64, i64, Score)] = &[
    (1, 1, Score::Like),
    (1, 3, Score::Like),
    (2, 1, Score::Dislike),
    (2, 4, Score::Like),
    (3, 1, Score::Dislike),
    (3, 3, Score::Dislike),
    (3, 4, Score::Dislike),
    (3, 5, Score::Dislike),
];

static CONNECTION_POOL: Mutex<Option<ConnectionPool>> = Mutex::new(None);

const fn new_user(name: &'static str, email: Option<&'static str>, rank: UserRank) -> NewUser<'static> {
    NewUser {
        name,
        password_hash: TEST_HASH,
        password_salt: TEST_SALT,
        email,
        rank,
        avatar_style: AvatarStyle::Manual,
    }
}

fn create_tag_categories(conn: &mut PgConnection) -> QueryResult<usize> {
    let new_categories: Vec<_> = TAG_CATEGORY_NAMES
        .iter()
        .enumerate()
        .map(|(i, name)| NewTagCategory {
            order: i as i32 + 1,
            name,
            color: "default",
        })
        .collect();
    diesel::insert_into(tag_category::table)
        .values(new_categories)
        .execute(conn)
}

fn create_tags(conn: &mut PgConnection) -> QueryResult<()> {
    for (i, tags) in TAG_GROUPS.iter().enumerate() {
        let new_tags: Vec<_> = tags
            .iter()
            .map(|_| NewTag {
                category_id: i as i64,
                description: "",
            })
            .collect();
        let tag_ids = diesel::insert_into(tag::table)
            .values(new_tags)
            .returning(tag::id)
            .get_results(conn)?;

        let new_tag_names: Vec<_> = tag_ids
            .iter()
            .zip(*tags)
            .flat_map(|(&tag_id, names)| {
                names.iter().enumerate().map(move |(i, name)| NewTagName {
                    tag_id,
                    order: i as i32,
                    name,
                })
            })
            .collect();
        diesel::insert_into(tag_name::table)
            .values(new_tag_names)
            .execute(conn)?;
    }

    for (parent, child) in TAG_IMPLICATIONS {
        let parent_id = tag::table
            .select(tag::id)
            .inner_join(tag_name::table)
            .filter(tag_name::name.eq(parent))
            .first(conn)?;
        let child_id = tag::table
            .select(tag::id)
            .inner_join(tag_name::table)
            .filter(tag_name::name.eq(child))
            .first(conn)?;
        let new_implication = TagImplication { parent_id, child_id };
        diesel::insert_into(tag_implication::table)
            .values(new_implication)
            .execute(conn)?;
    }
    for (parent, child) in TAG_SUGGESTIONS {
        let parent_id = tag::table
            .select(tag::id)
            .inner_join(tag_name::table)
            .filter(tag_name::name.eq(parent))
            .first(conn)?;
        let child_id = tag::table
            .select(tag::id)
            .inner_join(tag_name::table)
            .filter(tag_name::name.eq(child))
            .first(conn)?;
        let new_implication = TagSuggestion { parent_id, child_id };
        diesel::insert_into(tag_suggestion::table)
            .values(new_implication)
            .execute(conn)?;
    }
    Ok(())
}

fn create_pool_categories(conn: &mut PgConnection) -> QueryResult<usize> {
    let new_categories: Vec<_> = POOL_CATEGORY_NAMES
        .iter()
        .map(|name| NewPoolCategory { name, color: "default" })
        .collect();
    diesel::insert_into(pool_category::table)
        .values(new_categories)
        .execute(conn)
}

fn create_pools(conn: &mut PgConnection) -> QueryResult<()> {
    for (i, pools) in POOL_GROUPS.iter().enumerate() {
        let new_pools: Vec<_> = pools
            .iter()
            .map(|_| NewPool {
                category_id: i as i64,
                description: "",
            })
            .collect();
        let pool_ids = diesel::insert_into(pool::table)
            .values(new_pools)
            .returning(pool::id)
            .get_results(conn)?;

        let new_pool_names: Vec<_> = pool_ids
            .iter()
            .zip(*pools)
            .flat_map(|(&pool_id, names)| {
                names.iter().enumerate().map(move |(i, name)| NewPoolName {
                    pool_id,
                    order: i as i32,
                    name,
                })
            })
            .collect();
        diesel::insert_into(pool_name::table)
            .values(new_pool_names)
            .execute(conn)?;
    }
    Ok(())
}

fn populate_database(conn: &mut PgConnection) -> QueryResult<()> {
    // Create users
    diesel::insert_into(user::table).values(USERS).execute(conn)?;

    // Create user token
    let new_user_token = NewUserToken {
        id: TEST_TOKEN,
        user_id: 5,
        note: Some("This is a test token"),
        enabled: true,
        expiration_time: None,
    };
    diesel::insert_into(user_token::table)
        .values(new_user_token)
        .execute(conn)?;

    // Create tags and pools
    create_tag_categories(conn)?;
    create_tags(conn)?;
    create_pool_categories(conn)?;
    create_pools(conn)?;

    // Create posts
    diesel::insert_into(post::table).values(POSTS).execute(conn)?;

    // Add relations
    let new_post_relations: Vec<_> = POST_RELATIONS
        .iter()
        .flat_map(|&(id_1, id_2)| PostRelation::new_pair(id_1, id_2))
        .collect();
    diesel::insert_into(post_relation::table)
        .values(new_post_relations)
        .execute(conn)?;

    // Add tags
    for (i, &tags) in POST_TAGS.iter().enumerate() {
        let tag_ids = tag::table
            .select(tag::id)
            .distinct()
            .inner_join(tag_name::table)
            .filter(tag_name::name.eq_any(tags))
            .load(conn)?;
        let new_post_tags: Vec<_> = tag_ids
            .iter()
            .map(|&tag_id| PostTag {
                post_id: i as i64 + 1,
                tag_id,
            })
            .collect();
        diesel::insert_into(post_tag::table)
            .values(new_post_tags)
            .execute(conn)?;
    }

    // Add pools
    for (i, &(name, post_id)) in POOL_POSTS.iter().enumerate() {
        let pool_id = pool::table
            .select(pool::id)
            .inner_join(pool_name::table)
            .filter(pool_name::name.eq(name))
            .first(conn)?;
        let new_pool_post = PoolPost {
            pool_id,
            post_id,
            order: i as i64,
        };
        diesel::insert_into(pool_post::table)
            .values(new_pool_post)
            .execute(conn)?;
    }

    // Add favorites
    for &(user_id, post_id) in POST_FAVORITES {
        let new_post_favorite = PostFavorite {
            post_id,
            user_id,
            time: DateTime::now(),
        };
        diesel::insert_into(post_favorite::table)
            .values(new_post_favorite)
            .execute(conn)?;
    }

    // Add features
    for &(user_id, post_id) in POST_FEATURES {
        let new_post_feature = NewPostFeature {
            user_id,
            post_id,
            time: DateTime::now(),
        };
        diesel::insert_into(post_feature::table)
            .values(new_post_feature)
            .execute(conn)?;
    }

    // Add scores
    let new_scores: Vec<_> = POST_SCORES
        .iter()
        .map(|&(user_id, post_id, score)| PostScore {
            post_id,
            user_id,
            score,
            time: DateTime::now(),
        })
        .collect();
    diesel::insert_into(post_score::table)
        .values(new_scores)
        .execute(conn)?;

    // Add notes
    let post_note = NewPostNote {
        post_id: 3,
        polygon: &[0.0, 0.0, 0.0, 1.0, 1.0, 0.0],
        text: "My favorite part",
    };
    diesel::insert_into(post_note::table).values(post_note).execute(conn)?;

    // Add comments
    for &(user_id, post_id, text) in COMMENTS {
        let new_comment = NewComment {
            user_id,
            post_id,
            text,
            creation_time: DateTime::now(),
        };
        diesel::insert_into(comment::table).values(new_comment).execute(conn)?;
    }

    // Add comment scores
    let new_comment_scores: Vec<_> = COMMENT_SCORES
        .iter()
        .map(|&(comment_id, user_id, score)| NewCommentScore {
            comment_id,
            user_id,
            score,
        })
        .collect();
    diesel::insert_into(comment_score::table)
        .values(new_comment_scores)
        .execute(conn)?;

    Ok(())
}

fn recreate_database() -> Result<ConnectionPool, Box<dyn Error + Send + Sync>> {
    let mut conn = db::get_prod_connection()?;
    diesel::sql_query(format!("DROP DATABASE IF EXISTS {DATABASE_NAME}")).execute(&mut conn)?;
    diesel::sql_query(format!("CREATE DATABASE {DATABASE_NAME}")).execute(&mut conn)?;

    let database_url = db::create_url(Some(DATABASE_NAME));
    let mut conn = PgConnection::establish(&database_url).unwrap();
    db::run_migrations(&mut conn)?;
    populate_database(&mut conn)?;

    let manager = ConnectionManager::new(database_url);
    let pool = Pool::builder()
        .max_lifetime(Some(Duration::from_secs(60)))
        .idle_timeout(None)
        .test_on_check_out(true)
        .build(manager)
        .expect("Could not build connection pool");
    Ok(pool)
}

fn asset_path(folder_path: &str, relative_path: &str) -> PathBuf {
    let manifest_dir =
        std::env::var("CARGO_MANIFEST_DIR").expect("Test environment should have CARGO_MANIFEST_DIR defined");
    [&manifest_dir, "test", folder_path, relative_path].iter().collect()
}

mod test {
    use super::*;
    use crate::admin::database;
    use crate::model::pool::PoolName;
    use crate::model::tag::TagName;
    use crate::schema::{comment_statistics, database_statistics, user_statistics};
    use serial_test::{parallel, serial};

    #[test]
    #[parallel]
    fn database_statistics() -> ApiResult<()> {
        let expected_disk_usage: i64 = POSTS.iter().map(|post| post.file_size).sum();
        let expected_pool_count: i64 = POOL_GROUPS.iter().map(|group| group.len() as i64).sum();
        let expected_tag_count: i64 = TAG_GROUPS.iter().map(|group| group.len() as i64).sum();

        let mut conn = get_connection()?;
        let (id, disk_usage, comment_count, pool_count, post_count, tag_count, user_count, _sig_version): (
            bool,
            i64,
            i64,
            i64,
            i64,
            i64,
            i64,
            i32,
        ) = database_statistics::table.first(&mut conn)?;

        assert_eq!(id, true);
        assert_eq!(disk_usage, expected_disk_usage);
        assert_eq!(comment_count, COMMENTS.len() as i64);
        assert_eq!(pool_count, expected_pool_count);
        assert_eq!(post_count, POSTS.len() as i64);
        assert_eq!(tag_count, expected_tag_count);
        assert_eq!(user_count, USERS.len() as i64);
        Ok(())
    }

    #[test]
    #[parallel]
    fn comment_statistics() -> ApiResult<()> {
        let mut conn = get_connection()?;
        let stats: Vec<(i64, i64)> = comment_statistics::table.load(&mut conn)?;
        for (comment_id, total_score) in stats {
            let expected_score: i64 = COMMENT_SCORES
                .iter()
                .filter(|&&(id, ..)| id == comment_id)
                .map(|&(.., score)| score as i64)
                .sum();
            assert_eq!(total_score, expected_score);
        }
        Ok(())
    }

    #[test]
    #[parallel]
    fn pool_category_statistics() -> ApiResult<()> {
        let mut conn = get_connection()?;
        let stats: Vec<(i64, i64)> = pool_category_statistics::table.load(&mut conn)?;
        for (category_id, usage_count) in stats {
            let exepected_usage_count = POOL_GROUPS[category_id as usize].len() as i64;
            assert_eq!(usage_count, exepected_usage_count);
        }
        Ok(())
    }

    #[test]
    #[parallel]
    fn pool_statistics() -> ApiResult<()> {
        let mut conn = get_connection()?;
        let stats: Vec<(String, i64)> = pool_statistics::table
            .inner_join(pool_name::table.on(pool_name::pool_id.eq(pool_statistics::pool_id)))
            .select((pool_name::name, pool_statistics::post_count))
            .filter(PoolName::primary())
            .load(&mut conn)?;
        for (pool_name, post_count) in stats {
            let exepected_post_count = POOL_POSTS.iter().filter(|&&(name, _)| name == pool_name).count() as i64;
            assert_eq!(post_count, exepected_post_count);
        }
        Ok(())
    }

    #[test]
    #[parallel]
    fn post_statistics() -> ApiResult<()> {
        let mut conn = get_connection()?;
        let stats: Vec<(
            i64,
            i64,
            i64,
            i64,
            i64,
            i64,
            i64,
            i64,
            i64,
            Option<DateTime>,
            Option<DateTime>,
            Option<DateTime>,
        )> = post_statistics::table.load(&mut conn)?;

        for (
            post_id,
            tag_count,
            pool_count,
            note_count,
            comment_count,
            relation_count,
            score,
            favorite_count,
            feature_count,
            ..,
        ) in stats
        {
            let expected_tag_count = POST_TAGS[post_id as usize - 1].len() as i64;
            let expected_pool_count = POOL_POSTS.iter().filter(|&&(_, id)| id == post_id).count() as i64;
            let expected_note_count = if post_id == 3 { 1 } else { 0 };
            let expected_comment_count = COMMENTS.iter().filter(|&&(_, id, _)| id == post_id).count() as i64;
            let expected_relation_count = POST_RELATIONS
                .iter()
                .filter(|&&(id_1, id_2)| id_1 == post_id || id_2 == post_id)
                .count() as i64;
            let expected_score: i64 = POST_SCORES
                .iter()
                .filter(|&&(_, id, _)| id == post_id)
                .map(|&(.., score)| score as i64)
                .sum();
            let expected_favorite_count = POST_FAVORITES.iter().filter(|&&(_, id)| id == post_id).count() as i64;
            let expected_feature_count = POST_FEATURES.iter().filter(|&&(_, id)| id == post_id).count() as i64;

            assert_eq!(tag_count, expected_tag_count);
            assert_eq!(pool_count, expected_pool_count);
            assert_eq!(note_count, expected_note_count);
            assert_eq!(comment_count, expected_comment_count);
            assert_eq!(relation_count, expected_relation_count);
            assert_eq!(score, expected_score);
            assert_eq!(favorite_count, expected_favorite_count);
            assert_eq!(feature_count, expected_feature_count);
        }
        Ok(())
    }

    #[test]
    #[parallel]
    fn tag_category_statistics() -> ApiResult<()> {
        let mut conn = get_connection()?;
        let stats: Vec<(i64, i64)> = tag_category_statistics::table.load(&mut conn)?;
        for (category_id, usage_count) in stats {
            let expected_usage_count = TAG_GROUPS[category_id as usize].len() as i64;
            assert_eq!(usage_count, expected_usage_count);
        }
        Ok(())
    }

    #[test]
    #[parallel]
    fn tag_statistics() -> ApiResult<()> {
        let mut conn = get_connection()?;
        let stats: Vec<(String, i64)> = tag_statistics::table
            .inner_join(tag_name::table.on(tag_name::tag_id.eq(tag_statistics::tag_id)))
            .select((tag_name::name, tag_statistics::usage_count))
            .filter(TagName::primary())
            .load(&mut conn)?;
        for (tag_name, usage_count) in stats {
            let expected_usage_count = POST_TAGS
                .iter()
                .filter_map(|tags| tags.iter().find(|&&name| name == tag_name))
                .count() as i64;
            assert_eq!(usage_count, expected_usage_count);
        }
        Ok(())
    }

    #[test]
    #[parallel]
    fn user_statistics() -> ApiResult<()> {
        let mut conn = get_connection()?;
        let stats: Vec<(i64, i64, i64, i64)> = user_statistics::table.load(&mut conn)?;
        for (user_id, comment_count, favorite_count, upload_count) in stats {
            let expected_comment_count = COMMENTS.iter().filter(|&&(user, ..)| user == Some(user_id)).count() as i64;
            let expected_favorite_count = POST_FAVORITES.iter().filter(|&&(user, _)| user == user_id).count() as i64;
            let expected_upload_count = POSTS.iter().filter(|post| post.user_id == Some(user_id)).count() as i64;

            assert_eq!(comment_count, expected_comment_count);
            assert_eq!(favorite_count, expected_favorite_count);
            assert_eq!(upload_count, expected_upload_count);
        }
        Ok(())
    }

    #[test]
    #[serial]
    fn reset_statistics() -> ApiResult<()> {
        database::reset_relation_stats()?;
        database_statistics()?;
        comment_statistics()?;
        pool_category_statistics()?;
        pool_statistics()?;
        post_statistics()?;
        tag_category_statistics()?;
        tag_statistics()?;
        user_statistics()
    }
}
