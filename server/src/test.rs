use crate::admin::DatabaseResult;
use crate::api::ApiResult;
use crate::app::AppState;
use crate::auth::header;
use crate::config::Config;
use crate::content::hash::{Checksum, Md5Checksum, PostHash};
use crate::content::signature::{COMPRESSED_SIGNATURE_LEN, NUM_WORDS};
use crate::db::ConnectionResult;
use crate::model::comment::{NewComment, NewCommentScore};
use crate::model::enums::{
    AvatarStyle, MimeType, PostFlag, PostFlags, PostSafety, PostType, ResourceType, Score, UserRank,
};
use crate::model::pool::{NewPool, NewPoolName, PoolPost};
use crate::model::pool_category::NewPoolCategory;
use crate::model::post::{
    CompressedSignature, NewPost, NewPostFeature, NewPostNote, NewPostSignature, PostFavorite, PostRelation, PostScore,
    PostTag, SignatureIndexes,
};
use crate::model::tag::{NewTag, NewTagName, TagImplication, TagSuggestion};
use crate::model::tag_category::NewTagCategory;
use crate::model::user::{NewUser, NewUserToken};
use crate::schema::{
    comment, comment_score, pool, pool_category, pool_category_statistics, pool_name, pool_post, pool_statistics, post,
    post_favorite, post_feature, post_note, post_relation, post_score, post_signature, post_statistics, post_tag,
    snapshot, tag, tag_category, tag_category_statistics, tag_implication, tag_name, tag_statistics, tag_suggestion,
    user, user_token,
};
use crate::string::SmallString;
use crate::time::DateTime;
use crate::{api, config, db};
use argon2::password_hash::rand_core::{OsRng, RngCore};
use axum::ServiceExt;
use axum::extract::Request;
use axum::http::Method;
use axum::http::header::AUTHORIZATION;
use axum_test::TestServer;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::{ExpressionMethods, Insertable, JoinOnDsl, PgConnection, QueryDsl, QueryResult, RunQueryDsl};
use serde_json::Value;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use tower::layer::Layer;
use tower_http::normalize_path::NormalizePathLayer;
use uuid::Uuid;

pub const TEST_PASSWORD: &str = "test_password";
pub const TEST_SALT: &str = "test_salt";
pub const TEST_HASH: &str = "$argon2id$v=19$m=19456,t=2,p=1$dGVzdF9zYWx0$voqGcDZhS6JWiMJy9q12zBgrC6OTBKa9dL8k0O8gD4M";
pub const TEST_TOKEN: Uuid = uuid::uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8");

pub fn get_connection() -> ConnectionResult {
    get_state().connection_pool.get()
}

pub fn get_state() -> AppState {
    let mut guard = get_state_guard();
    if let Some(state) = guard.as_mut() {
        state.clone()
    } else {
        let state = recreate_database().expect("Test database must be constructible");
        *guard = Some(state.clone());
        state
    }
}

/// Resets the test database. Useful after operations that are hard to reverse perfectly, like merging.
pub fn reset_database() {
    *get_state_guard() = None;
}

/// Useful after a table's sequence gets updated as an unintended side-effect.
/// This can happen when attempting to insert a row but it fails due to a constraint violation.
pub fn reset_sequence(table: ResourceType) -> ApiResult<()> {
    let table: &str = table.into();
    let query = format!(
        "SELECT setval(pg_get_serial_sequence('{table}', 'id'), GREATEST((SELECT MAX(id) FROM \"{table}\"), 1));"
    );

    let mut conn = get_connection()?;
    diesel::sql_query(query).execute(&mut conn)?;
    Ok(())
}

/// Returns path to a test image.
pub fn image_path(relative_path: &str) -> PathBuf {
    asset_path("images", relative_path)
}

/// Verifies that a given `query` matches the contents of a `repsonse.json`
/// that lies in the `test/queries/relative_path` directory. A `snapshot.json`
/// will also be checked if it exists. A `body.json` and `config.toml` can be
/// specified if they exist.
///
/// `query` must be of the form `METHOD path` (e.g. `GET /post/1`).
pub async fn verify_query(query: &str, relative_path: &str) -> ApiResult<()> {
    verify_query_with_user(UserRank::Administrator, query, relative_path).await
}

pub async fn verify_query_with_user(user: UserRank, query: &str, relative_path: &str) -> ApiResult<()> {
    let credentials =
        (user != UserRank::Anonymous).then(|| header::credentials_for(USERS[user as usize - 1].name, TEST_PASSWORD));
    verify_query_with_credentials(credentials, query, relative_path).await
}

pub async fn verify_query_with_credentials(
    credentials: Option<String>,
    query: &str,
    relative_path: &str,
) -> ApiResult<()> {
    let mut expected_response: Option<Value> = None;
    let mut expected_snapshot: Option<Value> = None;
    let mut body: Option<Value> = None;
    let mut config: Option<Config> = None;
    for entry in std::fs::read_dir(asset_path("queries", relative_path))? {
        let path = entry?.path();
        let file_contents = std::fs::read_to_string(&path)?;
        match path.file_name().and_then(OsStr::to_str) {
            Some("response.json") => expected_response = Some(serde_json::from_str(&file_contents)?),
            Some("snapshot.json") => expected_snapshot = Some(serde_json::from_str(&file_contents)?),
            Some("body.json") => body = Some(serde_json::from_str(&file_contents)?),
            Some("config.toml") => config = Some(config::test_config(Some(relative_path))),
            Some(file_name) => panic!("Unexpected file name {file_name} in {relative_path}"),
            _ => panic!("Could not parse file name {:?} in {relative_path}", path.file_name()),
        }
    }

    // Optionally override default config
    let mut app_state = get_state();
    if let Some(mut config) = config {
        // Data directory should not be overriden
        config.data_dir = app_state.config.data_dir.clone();
        app_state.config = Arc::new(config);
    }

    let app = NormalizePathLayer::trim_trailing_slash().layer(api::routes(app_state));
    let (method, path) = query
        .split_once(' ')
        .expect("Query string must have method and path separated by a space");
    let method = Method::try_from(method).expect("Query string must start with a valid method");
    let path = path.replace(' ', "%20"); // Percent-encode all spaces

    let server =
        TestServer::new(ServiceExt::<Request>::into_make_service(app)).expect("Test server must be constructible");
    let mut request = server.method(method, &path);
    if let Some(credentials) = credentials {
        let basic_access_authentication = format!("Basic {credentials}");
        request = request.add_header(AUTHORIZATION, basic_access_authentication);
    }

    // Optionally specify a body
    let response = if let Some(body) = body {
        request.json(&body).await
    } else {
        request.await
    };

    // Optionally check an expected snapshot
    if let Some(expected_snapshot) = expected_snapshot {
        let mut conn = get_connection()?;
        let actual_snapshot: Value = snapshot::table
            .select(snapshot::data)
            .order_by(snapshot::id.desc())
            .first(&mut conn)?;
        verify_json(relative_path, "snapshot data", &expected_snapshot, &actual_snapshot);
    }

    if let Some(expected_response) = expected_response {
        let actual_response: Value = serde_json::from_slice(response.as_bytes()).unwrap_or_else(|_| {
            panic!("Response for {relative_path} is not JSON.\nBody:\n{}", String::from_utf8_lossy(response.as_bytes()))
        });
        verify_json(relative_path, "response body", &expected_response, &actual_response);
    } else {
        panic!("Missing response.json in {relative_path}");
    }
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

// NOTE: Source field must be the name of the coresponding image in test/images directory
const MB: i64 = 1024_i64.pow(2);
const GB: i64 = 1024_i64.pow(3);
const POSTS: &[NewPost] = &[
    NewPost {
        user_id: Some(1),
        file_size: MB,
        width: 1000,
        height: 2000,
        safety: PostSafety::Safe,
        type_: PostType::Image,
        mime_type: MimeType::Jpeg,
        checksum: Checksum::from_bytes(b"01"),
        checksum_md5: Md5Checksum::from_bytes(b"01"),
        flags: PostFlags::new(),
        source: "starry_night.png",
        description: "0101100010",
    },
    NewPost {
        user_id: Some(2),
        file_size: 5 * MB,
        width: 1980,
        height: 1080,
        safety: PostSafety::Sketchy,
        type_: PostType::Animation,
        mime_type: MimeType::Gif,
        checksum: Checksum::from_bytes(b"02"),
        checksum_md5: Md5Checksum::from_bytes(b"02"),
        flags: PostFlags::new(),
        source: "gif.gif",
        description: "",
    },
    NewPost {
        user_id: Some(2),
        file_size: 92 * MB,
        width: 11146,
        height: 7479,
        safety: PostSafety::Safe,
        type_: PostType::Image,
        mime_type: MimeType::Bmp,
        checksum: Checksum::from_bytes(b"03"),
        checksum_md5: Md5Checksum::from_bytes(b"03"),
        flags: PostFlags::new(),
        source: "bmp.bmp",
        description: "",
    },
    NewPost {
        user_id: Some(2),
        file_size: 1,
        width: 1,
        height: 1,
        safety: PostSafety::Safe,
        type_: PostType::Image,
        mime_type: MimeType::Png,
        checksum: Checksum::from_bytes(b"04"),
        checksum_md5: Md5Checksum::from_bytes(b"04"),
        flags: PostFlags::new(),
        source: "1_pixel.png",
        description: "description9000",
    },
    NewPost {
        user_id: None,
        file_size: 100 * GB,
        width: 1980,
        height: 1080,
        safety: PostSafety::Unsafe,
        type_: PostType::Video,
        mime_type: MimeType::Mp4,
        checksum: Checksum::from_bytes(b"05"),
        checksum_md5: Md5Checksum::from_bytes(b"05"),
        flags: PostFlags::new_with(PostFlag::Sound),
        source: "mp4.mp4",
        description: "descriptor",
    },
];

const POST_RELATIONS: &[(i64, i64)] = &[(1, 2), (1, 3), (4, 5)];

/// (`user_id`, `post_id`)
const POST_FAVORITES: &[(i64, i64)] = &[(1, 1), (2, 2), (2, 3), (2, 4), (5, 5)];

/// (`user_id`, `post_id`)
const POST_FEATURES: &[(i64, i64)] = &[(5, 5), (4, 4), (3, 1), (3, 3), (3, 1)];

/// (`user_id`, `post_id`, `score`)
const POST_SCORES: &[(i64, i64, Score)] = &[
    (1, 5, Score::Dislike),
    (2, 1, Score::Like),
    (2, 2, Score::Like),
    (2, 3, Score::Like),
    (4, 4, Score::Like),
    (5, 4, Score::Dislike),
    (5, 5, Score::Like),
];

/// (`user_id`, `post_id`, `text`)
const COMMENTS: &[(Option<i64>, i64, &str)] = &[
    (Some(2), 1, "Cool post!"),
    (Some(5), 1, "how did you post this"),
    (Some(2), 4, "I don't think this uploaded correctly"),
    (None, 5, "Lorem ipsum dolor sit amet, consectetur adipiscing elit"),
];

/// (`comment_id`, `user_id`, `score`)
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

static TEST_STATE: Mutex<Option<AppState>> = Mutex::new(None);

fn get_state_guard() -> MutexGuard<'static, Option<AppState>> {
    match TEST_STATE.lock() {
        Ok(guard) => guard,
        Err(err) => {
            // If panic occurs while holding lock, database may be in an invalid state
            eprintln!("Test database has been poisoned! Resetting...");
            let mut guard = err.into_inner();
            *guard = None;
            guard
        }
    }
}

fn recreate_database() -> DatabaseResult<AppState> {
    let rng = &mut OsRng;
    let test_data_directory = std::env::temp_dir().join(rng.next_u64().to_string());

    let mut test_config = config::test_config(None);
    test_config.data_dir = test_data_directory;

    // Drop and create test database via postgres database
    {
        let postgres_url = config::database_url(Some("postgres"));
        let postgres_connection_pool = Pool::builder()
            .max_size(1)
            .test_on_check_out(true)
            .build(ConnectionManager::<PgConnection>::new(postgres_url))
            .expect("Postgres connection pool must be constructible");

        let mut conn = postgres_connection_pool.get()?;
        diesel::sql_query(format!("DROP DATABASE IF EXISTS {DATABASE_NAME}")).execute(&mut conn)?;
        diesel::sql_query(format!("CREATE DATABASE {DATABASE_NAME}")).execute(&mut conn)?;
    }

    let test_url = config::database_url(Some(DATABASE_NAME));
    let test_connection_pool = Pool::builder()
        .max_lifetime(None)
        .idle_timeout(None)
        .test_on_check_out(true)
        .build(ConnectionManager::new(test_url))
        .expect("Test connection pool must be constructible");
    db::run_database_migrations(&test_connection_pool).expect("Must be able to run test migrations");

    let mut conn = test_connection_pool.get()?;
    populate_database(&mut conn, &test_config)?;
    Ok(AppState::new(test_connection_pool, test_config))
}

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
    new_categories.insert_into(tag_category::table).execute(conn)
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
        let tag_ids = new_tags.insert_into(tag::table).returning(tag::id).get_results(conn)?;

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
        new_tag_names.insert_into(tag_name::table).execute(conn)?;
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
        TagImplication { parent_id, child_id }
            .insert_into(tag_implication::table)
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
        TagSuggestion { parent_id, child_id }
            .insert_into(tag_suggestion::table)
            .execute(conn)?;
    }
    Ok(())
}

fn create_pool_categories(conn: &mut PgConnection) -> QueryResult<usize> {
    let new_categories: Vec<_> = POOL_CATEGORY_NAMES
        .iter()
        .map(|name| NewPoolCategory { name, color: "default" })
        .collect();
    new_categories.insert_into(pool_category::table).execute(conn)
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
        let pool_ids = new_pools
            .insert_into(pool::table)
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
        new_pool_names.insert_into(pool_name::table).execute(conn)?;
    }
    Ok(())
}

fn create_posts(conn: &mut PgConnection, config: &Config) -> DatabaseResult<()> {
    let post_data: Vec<(i64, MimeType, String)> = POSTS
        .insert_into(post::table)
        .returning((post::id, post::mime_type, post::source))
        .get_results(conn)?;

    // Simulate uploads
    for (post_id, mime_type, source) in post_data {
        let post_signature = NewPostSignature {
            post_id,
            signature: CompressedSignature::from([0; COMPRESSED_SIGNATURE_LEN]),
            words: SignatureIndexes::from([0; NUM_WORDS]),
        };
        diesel::insert_into(post_signature::table)
            .values(post_signature)
            .execute(conn)?;

        let post_hash = PostHash::new(config, post_id);
        let content_path = post_hash.content_path(mime_type);
        std::fs::create_dir_all(content_path.parent().unwrap_or(Path::new("")))?;

        std::fs::copy(image_path(&source), content_path)?;
    }
    Ok(())
}

fn populate_database(conn: &mut PgConnection, config: &Config) -> DatabaseResult<()> {
    // Create users
    USERS.insert_into(user::table).execute(conn)?;

    // Create user token
    NewUserToken {
        id: TEST_TOKEN,
        user_id: 5,
        note: Some("This is a test token"),
        enabled: true,
        expiration_time: None,
    }
    .insert_into(user_token::table)
    .execute(conn)?;

    // Create tags and pools
    create_tag_categories(conn)?;
    create_tags(conn)?;
    create_pool_categories(conn)?;
    create_pools(conn)?;
    create_posts(conn, config)?;

    // Add relations
    let new_post_relations: Vec<_> = POST_RELATIONS
        .iter()
        .flat_map(|&(id_1, id_2)| PostRelation::new_pair(id_1, id_2))
        .collect();
    new_post_relations.insert_into(post_relation::table).execute(conn)?;

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
        new_post_tags.insert_into(post_tag::table).execute(conn)?;
    }

    // Add pools
    for (i, &(name, post_id)) in POOL_POSTS.iter().enumerate() {
        let pool_id = pool::table
            .select(pool::id)
            .inner_join(pool_name::table)
            .filter(pool_name::name.eq(name))
            .first(conn)?;
        PoolPost {
            pool_id,
            post_id,
            order: i as i64,
        }
        .insert_into(pool_post::table)
        .execute(conn)?;
    }

    // Add favorites
    for &(user_id, post_id) in POST_FAVORITES {
        PostFavorite {
            post_id,
            user_id,
            time: DateTime::now(),
        }
        .insert_into(post_favorite::table)
        .execute(conn)?;
    }

    // Add features
    for &(user_id, post_id) in POST_FEATURES {
        NewPostFeature {
            user_id,
            post_id,
            time: DateTime::now(),
        }
        .insert_into(post_feature::table)
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
    new_scores.insert_into(post_score::table).execute(conn)?;

    // Add notes
    NewPostNote {
        post_id: 3,
        polygon: &[0.0, 0.0, 0.0, 1.0, 1.0, 0.0],
        text: "My favorite part",
    }
    .insert_into(post_note::table)
    .execute(conn)?;

    // Add comments
    for &(user_id, post_id, text) in COMMENTS {
        NewComment {
            user_id,
            post_id,
            text,
            creation_time: DateTime::now(),
        }
        .insert_into(comment::table)
        .execute(conn)?;
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
    new_comment_scores.insert_into(comment_score::table).execute(conn)?;

    Ok(())
}

fn asset_path(folder_path: &str, relative_path: &str) -> PathBuf {
    let manifest_dir =
        std::env::var("CARGO_MANIFEST_DIR").expect("Test environment must have CARGO_MANIFEST_DIR defined");
    [&manifest_dir, "test", folder_path, relative_path].iter().collect()
}

fn verify_json(test_name: &str, json_type: &str, expected: &Value, actual: &Value) {
    let diff_message = crate::snapshot::value_diff(expected.clone(), actual.clone())
        .map_or("JSON contents are nearly equal, but at least one array element is out-of-order!".into(), |diff| {
            format!("Diff:\n{diff}")
        });
    assert!(
        expected == actual,
        "Incorrect {json_type} for {test_name}. Expected:\n{expected}\n\nReceived:\n{actual}\n\n{diff_message}\n",
    );
}

mod statistics_tests {
    use super::*;
    use crate::admin::database;
    use crate::model::pool::PoolName;
    use crate::model::tag::TagName;
    use crate::schema::{comment_statistics, database_statistics, user_statistics};
    use serial_test::{parallel, serial};

    #[test]
    #[parallel]
    fn database_statistics() -> DatabaseResult<()> {
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

        assert!(id);
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
    fn comment_statistics() -> DatabaseResult<()> {
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
    fn pool_category_statistics() -> DatabaseResult<()> {
        let mut conn = get_connection()?;
        let stats: Vec<(i64, i64)> = pool_category_statistics::table.load(&mut conn)?;
        for (category_id, usage_count) in stats {
            let exepected_usage_count = POOL_GROUPS[usize::try_from(category_id).unwrap()].len() as i64;
            assert_eq!(usage_count, exepected_usage_count);
        }
        Ok(())
    }

    #[test]
    #[parallel]
    fn pool_statistics() -> DatabaseResult<()> {
        let mut conn = get_connection()?;
        let stats: Vec<(SmallString, i64)> = pool_statistics::table
            .inner_join(pool_name::table.on(pool_name::pool_id.eq(pool_statistics::pool_id)))
            .select((pool_name::name, pool_statistics::post_count))
            .filter(PoolName::primary())
            .load(&mut conn)?;
        for (pool_name, post_count) in stats {
            let exepected_post_count = POOL_POSTS.iter().filter(|&&(name, _)| *name == *pool_name).count() as i64;
            assert_eq!(post_count, exepected_post_count);
        }
        Ok(())
    }

    #[test]
    #[parallel]
    fn post_statistics() -> DatabaseResult<()> {
        type PostData =
            (i64, i64, i64, i64, i64, i64, i64, i64, i64, Option<DateTime>, Option<DateTime>, Option<DateTime>);

        let mut conn = get_connection()?;
        let stats: Vec<PostData> = post_statistics::table.load(&mut conn)?;
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
            let expected_tag_count = POST_TAGS[usize::try_from(post_id - 1).unwrap()].len() as i64;
            let expected_pool_count = POOL_POSTS.iter().filter(|&&(_, id)| id == post_id).count() as i64;
            let expected_note_count = i64::from(post_id == 3);
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
    fn tag_category_statistics() -> DatabaseResult<()> {
        let mut conn = get_connection()?;
        let stats: Vec<(i64, i64)> = tag_category_statistics::table.load(&mut conn)?;
        for (category_id, usage_count) in stats {
            let expected_usage_count = TAG_GROUPS[usize::try_from(category_id).unwrap()].len() as i64;
            assert_eq!(usage_count, expected_usage_count);
        }
        Ok(())
    }

    #[test]
    #[parallel]
    fn tag_statistics() -> DatabaseResult<()> {
        let mut conn = get_connection()?;
        let stats: Vec<(SmallString, i64)> = tag_statistics::table
            .inner_join(tag_name::table.on(tag_name::tag_id.eq(tag_statistics::tag_id)))
            .select((tag_name::name, tag_statistics::usage_count))
            .filter(TagName::primary())
            .load(&mut conn)?;
        for (tag_name, usage_count) in stats {
            let expected_usage_count = POST_TAGS
                .iter()
                .filter_map(|tags| tags.iter().find(|&&name| *name == *tag_name))
                .count() as i64;
            assert_eq!(usage_count, expected_usage_count);
        }
        Ok(())
    }

    #[test]
    #[parallel]
    fn user_statistics() -> DatabaseResult<()> {
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
    fn reset_statistics() -> DatabaseResult<()> {
        database::reset_relation_stats(&get_state())?;
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
