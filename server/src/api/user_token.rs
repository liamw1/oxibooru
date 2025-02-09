use crate::api::{ApiResult, AuthResult, ResourceQuery, UnpagedResponse};
use crate::model::enums::AvatarStyle;
use crate::model::user::{NewUserToken, UserToken};
use crate::resource::user::MicroUser;
use crate::resource::user_token::{Field, UserTokenInfo};
use crate::schema::{user, user_token};
use crate::time::DateTime;
use crate::{api, config, db};
use diesel::prelude::*;
use serde::Deserialize;
use uuid::Uuid;
use warp::{Filter, Rejection, Reply};

pub fn routes() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    let list = warp::get()
        .and(api::auth())
        .and(warp::path!("user-tokens" / String))
        .and(api::resource_query())
        .map(list)
        .map(api::Reply::from);
    let create = warp::post()
        .and(api::auth())
        .and(warp::path!("user-token" / String))
        .and(api::resource_query())
        .and(warp::body::json())
        .map(create)
        .map(api::Reply::from);
    let update = warp::put()
        .and(api::auth())
        .and(warp::path!("user-token" / String / Uuid))
        .and(api::resource_query())
        .and(warp::body::json())
        .map(update)
        .map(api::Reply::from);
    let delete = warp::delete()
        .and(api::auth())
        .and(warp::path!("user-token" / String / Uuid))
        .map(delete)
        .map(api::Reply::from);

    list.or(create).or(update).or(delete)
}

fn list(auth: AuthResult, username: String, query: ResourceQuery) -> ApiResult<UnpagedResponse<UserTokenInfo>> {
    let client = auth?;
    let username = percent_encoding::percent_decode_str(&username).decode_utf8()?;
    let fields = Field::create_table(query.fields()).map_err(Box::from)?;
    db::get_connection()?.transaction(|conn| {
        let (user_id, avatar_style): (i64, AvatarStyle) = user::table
            .select((user::id, user::avatar_style))
            .filter(user::name.eq(&username))
            .first(conn)?;

        let required_rank = match client.id == Some(user_id) {
            true => config::privileges().user_token_list_self,
            false => config::privileges().user_token_list_any,
        };
        api::verify_privilege(client, required_rank)?;

        let results = user_token::table
            .filter(user_token::user_id.eq(user_id))
            .load(conn)?
            .into_iter()
            .map(|user_token| {
                UserTokenInfo::new(MicroUser::new(username.to_string(), avatar_style), user_token, &fields)
            })
            .collect();
        Ok(UnpagedResponse { results })
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
struct NewUserTokenInfo {
    enabled: bool,
    note: Option<String>,
    expiration_time: Option<DateTime>,
}

fn create(
    auth: AuthResult,
    username: String,
    query: ResourceQuery,
    token_info: NewUserTokenInfo,
) -> ApiResult<UserTokenInfo> {
    let client = auth?;
    let username = percent_encoding::percent_decode_str(&username).decode_utf8()?;
    let fields = Field::create_table(query.fields()).map_err(Box::from)?;

    let mut conn = db::get_connection()?;
    let (user_token, avatar_style) = conn.transaction(|conn| {
        let (user_id, avatar_style): (i64, AvatarStyle) = user::table
            .select((user::id, user::avatar_style))
            .filter(user::name.eq(&username))
            .first(conn)?;

        let required_rank = match client.id == Some(user_id) {
            true => config::privileges().user_token_create_self,
            false => config::privileges().user_token_create_any,
        };
        api::verify_privilege(client, required_rank)?;

        // Delete any expired or disabled tokens owned by user
        let current_time = DateTime::now();
        diesel::delete(user_token::table)
            .filter(user_token::user_id.eq(user_id))
            .filter(
                user_token::enabled
                    .eq(false)
                    .or(user_token::expiration_time.lt(current_time)),
            )
            .execute(conn)?;

        let new_user_token = NewUserToken {
            id: Uuid::new_v4(),
            user_id,
            note: token_info.note.as_deref(),
            enabled: token_info.enabled,
            expiration_time: token_info.expiration_time,
        };
        let user_token = diesel::insert_into(user_token::table)
            .values(new_user_token)
            .returning(UserToken::as_returning())
            .get_result(conn)?;
        Ok::<_, api::Error>((user_token, avatar_style))
    })?;
    Ok(UserTokenInfo::new(MicroUser::new(username.to_string(), avatar_style), user_token, &fields))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
struct UserTokenUpdate {
    version: DateTime,
    enabled: Option<bool>,
    note: Option<String>,
    #[serde(default, deserialize_with = "api::deserialize_some")]
    expiration_time: Option<Option<DateTime>>,
}

fn update(
    auth: AuthResult,
    username: String,
    token: Uuid,
    query: ResourceQuery,
    update: UserTokenUpdate,
) -> ApiResult<UserTokenInfo> {
    let client = auth?;
    let username = percent_encoding::percent_decode_str(&username).decode_utf8()?;
    let fields = Field::create_table(query.fields()).map_err(Box::from)?;

    let mut conn = db::get_connection()?;
    let (updated_user_token, avatar_style) = conn.transaction(|conn| {
        let (user_id, avatar_style): (i64, AvatarStyle) = user::table
            .select((user::id, user::avatar_style))
            .filter(user::name.eq(&username))
            .first(conn)?;

        let required_rank = match client.id == Some(user_id) {
            true => config::privileges().user_token_edit_self,
            false => config::privileges().user_token_edit_any,
        };
        api::verify_privilege(client, required_rank)?;

        let mut user_token: UserToken = user_token::table.find(token).first(conn)?;
        api::verify_version(user_token.last_edit_time, update.version)?;

        if let Some(enabled) = update.enabled {
            user_token.enabled = enabled;
        }
        if let Some(note) = update.note {
            user_token.note = note;
        }
        if let Some(expiration_time) = update.expiration_time {
            user_token.expiration_time = expiration_time;
        }
        user_token.last_edit_time = DateTime::now();

        let updated_user_token: UserToken = user_token.save_changes(conn)?;
        Ok::<_, api::Error>((updated_user_token, avatar_style))
    })?;
    Ok(UserTokenInfo::new(MicroUser::new(username.to_string(), avatar_style), updated_user_token, &fields))
}

fn delete(auth: AuthResult, username: String, token: Uuid) -> ApiResult<()> {
    let client = auth?;
    let username = percent_encoding::percent_decode_str(&username).decode_utf8()?;
    db::get_connection()?.transaction(|conn| {
        let user_token_owner: i64 = user::table
            .inner_join(user_token::table)
            .select(user_token::user_id)
            .filter(user::name.eq(username))
            .filter(user_token::id.eq(token))
            .first(conn)?;

        let required_rank = match client.id == Some(user_token_owner) {
            true => config::privileges().user_token_delete_self,
            false => config::privileges().user_token_delete_any,
        };
        api::verify_privilege(client, required_rank)?;

        diesel::delete(user_token::table.find(token)).execute(conn)?;
        Ok(())
    })
}

#[cfg(test)]
mod test {
    use crate::api::ApiResult;
    use crate::model::user::UserToken;
    use crate::schema::{user, user_token};
    use crate::test::*;
    use diesel::dsl::exists;
    use diesel::prelude::*;
    use serial_test::{parallel, serial};
    use uuid::Uuid;

    // Exclude fields that involve token, creation_time, last_edit_time, or last_usage_time
    const FIELDS: &str = "&fields=user,note,enabled,expirationTime";

    #[tokio::test]
    #[parallel]
    async fn list() -> ApiResult<()> {
        const USER: &str = "administrator";
        verify_query(&format!("GET /user-tokens/{USER}/?{FIELDS}"), "user_token/list.json").await
    }

    #[tokio::test]
    #[serial]
    async fn create() -> ApiResult<()> {
        const USER: &str = "restricted_user";
        verify_query(&format!("POST /user-token/{USER}/?{FIELDS}"), "user_token/create.json").await?;

        let mut conn = get_connection()?;
        let token: Uuid = user_token::table
            .select(user_token::id)
            .order_by(user_token::creation_time.desc())
            .first(&mut conn)?;

        verify_query(&format!("DELETE /user-token/{USER}/{token}"), "delete.json").await?;

        let has_token: bool = diesel::select(exists(user_token::table.find(token))).get_result(&mut conn)?;
        assert!(!has_token);
        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn update() -> ApiResult<()> {
        const USER: &str = "administrator";
        let get_user_token = |conn: &mut PgConnection| -> QueryResult<UserToken> {
            user::table
                .inner_join(user_token::table)
                .select(UserToken::as_select())
                .filter(user::name.eq(USER))
                .first(conn)
        };

        let mut conn = get_connection()?;
        let user_token = get_user_token(&mut conn)?;

        verify_query(&format!("PUT /user-token/{USER}/{TEST_TOKEN}/?{FIELDS}"), "user_token/update.json").await?;

        let new_user_token = get_user_token(&mut conn)?;
        assert_eq!(new_user_token.id, user_token.id);
        assert_eq!(new_user_token.user_id, user_token.user_id);
        assert_ne!(new_user_token.note, user_token.note);
        assert_ne!(new_user_token.enabled, user_token.enabled);
        assert_ne!(new_user_token.expiration_time, user_token.expiration_time);
        assert!(new_user_token.last_edit_time > user_token.last_edit_time);
        assert_eq!(new_user_token.last_usage_time, user_token.last_usage_time);

        verify_query(&format!("PUT /user-token/{USER}/{TEST_TOKEN}/?{FIELDS}"), "user_token/update_restore.json")
            .await?;

        let new_user_token = get_user_token(&mut conn)?;
        assert_eq!(new_user_token.id, user_token.id);
        assert_eq!(new_user_token.user_id, user_token.user_id);
        assert_eq!(new_user_token.note, user_token.note);
        assert_eq!(new_user_token.enabled, user_token.enabled);
        assert_eq!(new_user_token.expiration_time, user_token.expiration_time);
        assert!(new_user_token.last_edit_time > user_token.last_edit_time);
        assert_eq!(new_user_token.last_usage_time, user_token.last_usage_time);
        Ok(())
    }
}
