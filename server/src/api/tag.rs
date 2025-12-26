use crate::api::error::{ApiError, ApiResult};
use crate::api::extract::{Json, Path, Query};
use crate::api::{DeleteBody, MergeBody, PageParams, PagedResponse, ResourceParams};
use crate::app::AppState;
use crate::auth::Client;
use crate::model::enums::ResourceType;
use crate::model::tag::{NewTag, Tag};
use crate::resource::tag::TagInfo;
use crate::schema::{post_tag, tag, tag_category, tag_name};
use crate::search::Builder;
use crate::search::tag::QueryBuilder;
use crate::snapshot::tag::SnapshotData;
use crate::string::{LargeString, SmallString};
use crate::time::DateTime;
use crate::{api, resource, snapshot, update};
use axum::extract::{Extension, State};
use axum::{Router, routing};
use diesel::dsl::count_star;
use diesel::{
    Connection, ExpressionMethods, Insertable, OptionalExtension, PgConnection, QueryDsl, RunQueryDsl, SaveChangesDsl,
    SelectableHelper,
};
use serde::{Deserialize, Serialize};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/tags", routing::get(list).post(create))
        .route("/tag/{name}", routing::get(get).put(update).delete(delete))
        .route("/tag-siblings/{name}", routing::get(get_siblings))
        .route("/tag-merge", routing::post(merge))
}

const MAX_TAGS_PER_PAGE: i64 = 1000;
const MAX_TAG_SIBLINGS: i64 = 1000;

/// See [listing-tags](https://github.com/liamw1/oxibooru/blob/master/docs/API.md#listing-tags)
async fn list(
    State(state): State<AppState>,
    Extension(client): Extension<Client>,
    Query(params): Query<PageParams>,
) -> ApiResult<Json<PagedResponse<TagInfo>>> {
    api::verify_privilege(client, state.config.privileges().tag_list)?;

    let offset = params.offset.unwrap_or(0);
    let limit = std::cmp::min(params.limit.get(), MAX_TAGS_PER_PAGE);
    let fields = resource::create_table(params.fields()).map_err(Box::from)?;

    state.get_connection()?.transaction(|conn| {
        let mut query_builder = QueryBuilder::new(client, params.criteria())?;
        query_builder.set_offset_and_limit(offset, limit);

        let (total, selected_tags) = query_builder.list(conn)?;
        Ok(Json(PagedResponse {
            query: params.into_query(),
            offset,
            limit,
            total,
            results: TagInfo::new_batch_from_ids(conn, &selected_tags, &fields)?,
        }))
    })
}

/// See [getting-tag](https://github.com/liamw1/oxibooru/blob/master/docs/API.md#getting-tag)
async fn get(
    State(state): State<AppState>,
    Extension(client): Extension<Client>,
    Path(name): Path<String>,
    Query(params): Query<ResourceParams>,
) -> ApiResult<Json<TagInfo>> {
    api::verify_privilege(client, state.config.privileges().tag_view)?;

    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    state.get_connection()?.transaction(|conn| {
        let tag_id = tag_name::table
            .select(tag_name::tag_id)
            .filter(tag_name::name.eq(name))
            .first(conn)
            .optional()?
            .ok_or(ApiError::NotFound(ResourceType::Tag))?;
        TagInfo::new_from_id(conn, tag_id, &fields)
            .map(Json)
            .map_err(ApiError::from)
    })
}

#[derive(Serialize)]
struct TagSibling {
    tag: TagInfo,
    occurrences: i64,
}

#[derive(Serialize)]
struct TagSiblings {
    results: Vec<TagSibling>,
}

/// See [getting-tag-siblings](https://github.com/liamw1/oxibooru/blob/master/docs/API.md#getting-tag-siblings)
async fn get_siblings(
    State(state): State<AppState>,
    Extension(client): Extension<Client>,
    Path(name): Path<String>,
    Query(params): Query<ResourceParams>,
) -> ApiResult<Json<TagSiblings>> {
    api::verify_privilege(client, state.config.privileges().tag_view)?;

    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    state.get_connection()?.transaction(|conn| {
        let tag_id: i64 = tag::table
            .select(tag::id)
            .inner_join(tag_name::table)
            .filter(tag_name::name.eq(name))
            .first(conn)
            .optional()?
            .ok_or(ApiError::NotFound(ResourceType::Tag))?;
        let posts_tagged_on = post_tag::table
            .select(post_tag::post_id)
            .filter(post_tag::tag_id.eq(tag_id))
            .into_boxed();
        let (sibling_ids, common_post_counts): (Vec<_>, Vec<_>) = post_tag::table
            .group_by(post_tag::tag_id)
            .select((post_tag::tag_id, count_star()))
            .filter(post_tag::post_id.eq_any(posts_tagged_on))
            .filter(post_tag::tag_id.ne(tag_id))
            .order_by((count_star().desc(), post_tag::tag_id))
            .limit(MAX_TAG_SIBLINGS)
            .load::<(i64, i64)>(conn)?
            .into_iter()
            .unzip();

        let siblings = TagInfo::new_batch_from_ids(conn, &sibling_ids, &fields)?
            .into_iter()
            .zip(common_post_counts)
            .map(|(tag, occurrences)| TagSibling { tag, occurrences })
            .collect();
        Ok(Json(TagSiblings { results: siblings }))
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateBody {
    category: SmallString,
    description: Option<LargeString>,
    names: Vec<SmallString>,
    implications: Option<Vec<SmallString>>,
    suggestions: Option<Vec<SmallString>>,
}

/// See [creating-tag](https://github.com/liamw1/oxibooru/blob/master/docs/API.md#creating-tag)
async fn create(
    State(state): State<AppState>,
    Extension(client): Extension<Client>,
    Query(params): Query<ResourceParams>,
    Json(body): Json<CreateBody>,
) -> ApiResult<Json<TagInfo>> {
    api::verify_privilege(client, state.config.privileges().tag_create)?;

    if body.names.is_empty() {
        return Err(ApiError::NoNamesGiven(ResourceType::Tag));
    }

    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    let mut conn = state.get_connection()?;
    let tag = conn.transaction(|conn| {
        let (category_id, category): (i64, SmallString) = tag_category::table
            .select((tag_category::id, tag_category::name))
            .filter(tag_category::name.eq(body.category))
            .first(conn)
            .optional()?
            .ok_or(ApiError::NotFound(ResourceType::TagCategory))?;
        let tag: Tag = NewTag {
            category_id,
            description: body.description.as_deref().unwrap_or(""),
        }
        .insert_into(tag::table)
        .get_result(conn)?;

        // Add names, implications, and suggestions
        update::tag::set_names(conn, &state.config, tag.id, &body.names)?;
        let (implied_ids, implications) = update::tag::get_or_create_tag_ids(
            conn,
            &state.config,
            client,
            body.implications.unwrap_or_default(),
            true,
        )?;
        let (suggested_ids, suggestions) = update::tag::get_or_create_tag_ids(
            conn,
            &state.config,
            client,
            body.suggestions.unwrap_or_default(),
            true,
        )?;
        update::tag::set_implications(conn, tag.id, &implied_ids)?;
        update::tag::set_suggestions(conn, tag.id, &suggested_ids)?;

        let tag_data = SnapshotData {
            description: body.description.unwrap_or_default(),
            category,
            names: body.names,
            implications,
            suggestions,
        };
        snapshot::tag::creation_snapshot(conn, client, tag_data)?;
        Ok::<_, ApiError>(tag)
    })?;
    conn.transaction(|conn| TagInfo::new(conn, tag, &fields))
        .map(Json)
        .map_err(ApiError::from)
}

/// See [merging-tags](https://github.com/liamw1/oxibooru/blob/master/docs/API.md#merging-tags)
async fn merge(
    State(state): State<AppState>,
    Extension(client): Extension<Client>,
    Query(params): Query<ResourceParams>,
    Json(body): Json<MergeBody<SmallString>>,
) -> ApiResult<Json<TagInfo>> {
    api::verify_privilege(client, state.config.privileges().tag_merge)?;

    let get_tag_info = |conn: &mut PgConnection, name: &str| {
        tag::table
            .select((tag::id, tag::last_edit_time))
            .inner_join(tag_name::table)
            .filter(tag_name::name.eq(name))
            .first(conn)
            .optional()?
            .ok_or(ApiError::NotFound(ResourceType::Tag))
    };

    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    let mut conn = state.get_connection()?;
    let merged_tag_id = conn.transaction(|conn| {
        let (absorbed_id, absorbed_version) = get_tag_info(conn, &body.remove)?;
        let (merge_to_id, merge_to_version) = get_tag_info(conn, &body.merge_to)?;
        if absorbed_id == merge_to_id {
            return Err(ApiError::SelfMerge(ResourceType::Tag));
        }
        api::verify_version(absorbed_version, body.remove_version)?;
        api::verify_version(merge_to_version, body.merge_to_version)?;

        update::tag::merge(conn, absorbed_id, merge_to_id)?;
        snapshot::tag::merge_snapshot(conn, client, body.remove, &body.merge_to)?;
        Ok::<_, ApiError>(merge_to_id)
    })?;
    conn.transaction(|conn| TagInfo::new_from_id(conn, merged_tag_id, &fields))
        .map(Json)
        .map_err(ApiError::from)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdateBody {
    version: DateTime,
    category: Option<SmallString>,
    description: Option<LargeString>,
    names: Option<Vec<SmallString>>,
    implications: Option<Vec<SmallString>>,
    suggestions: Option<Vec<SmallString>>,
}

/// See [updating-tag](https://github.com/liamw1/oxibooru/blob/master/docs/API.md#updating-tag)
async fn update(
    State(state): State<AppState>,
    Extension(client): Extension<Client>,
    Path(name): Path<String>,
    Query(params): Query<ResourceParams>,
    Json(body): Json<UpdateBody>,
) -> ApiResult<Json<TagInfo>> {
    let fields = resource::create_table(params.fields()).map_err(Box::from)?;

    let mut conn = state.get_connection()?;
    let tag_id = conn.transaction(|conn| {
        let old_tag: Tag = tag::table
            .inner_join(tag_name::table)
            .select(Tag::as_select())
            .filter(tag_name::name.eq(name))
            .first(conn)
            .optional()?
            .ok_or(ApiError::NotFound(ResourceType::Tag))?;
        let tag_id = old_tag.id;
        api::verify_version(old_tag.last_edit_time, body.version)?;

        let mut new_tag = old_tag.clone();
        let old_snapshot_data = SnapshotData::retrieve(conn, old_tag)?;
        let mut new_snapshot_data = old_snapshot_data.clone();

        if let Some(category) = body.category {
            api::verify_privilege(client, state.config.privileges().tag_edit_category)?;

            let category_id: i64 = tag_category::table
                .select(tag_category::id)
                .filter(tag_category::name.eq(&category))
                .first(conn)
                .optional()?
                .ok_or(ApiError::NotFound(ResourceType::TagCategory))?;
            new_tag.category_id = category_id;
            new_snapshot_data.category = category;
        }
        if let Some(description) = body.description {
            api::verify_privilege(client, state.config.privileges().tag_edit_description)?;
            new_tag.description = description.clone();
            new_snapshot_data.description = description;
        }
        if let Some(names) = body.names {
            api::verify_privilege(client, state.config.privileges().tag_edit_name)?;
            if names.is_empty() {
                return Err(ApiError::NoNamesGiven(ResourceType::Tag));
            }

            update::tag::set_names(conn, &state.config, tag_id, &names)?;
            new_snapshot_data.names = names;
        }
        if let Some(implications) = body.implications {
            api::verify_privilege(client, state.config.privileges().tag_edit_implication)?;

            let (implied_ids, implications) =
                update::tag::get_or_create_tag_ids(conn, &state.config, client, implications, true)?;
            update::tag::set_implications(conn, tag_id, &implied_ids)?;
            new_snapshot_data.implications = implications;
        }
        if let Some(suggestions) = body.suggestions {
            api::verify_privilege(client, state.config.privileges().tag_edit_suggestion)?;

            let (suggested_ids, suggestions) =
                update::tag::get_or_create_tag_ids(conn, &state.config, client, suggestions, true)?;
            update::tag::set_suggestions(conn, tag_id, &suggested_ids)?;
            new_snapshot_data.suggestions = suggestions;
        }

        new_tag.last_edit_time = DateTime::now();
        let _: Tag = new_tag.save_changes(conn)?;
        snapshot::tag::modification_snapshot(conn, client, old_snapshot_data, new_snapshot_data)?;
        Ok::<_, ApiError>(tag_id)
    })?;
    conn.transaction(|conn| TagInfo::new_from_id(conn, tag_id, &fields))
        .map(Json)
        .map_err(ApiError::from)
}

/// See [deleting-tag](https://github.com/liamw1/oxibooru/blob/master/docs/API.md#deleting-tag)
async fn delete(
    State(state): State<AppState>,
    Extension(client): Extension<Client>,
    Path(name): Path<String>,
    Json(client_version): Json<DeleteBody>,
) -> ApiResult<Json<()>> {
    api::verify_privilege(client, state.config.privileges().tag_delete)?;

    state.get_connection()?.transaction(|conn| {
        let tag: Tag = tag::table
            .select(Tag::as_select())
            .inner_join(tag_name::table)
            .filter(tag_name::name.eq(name))
            .first(conn)
            .optional()?
            .ok_or(ApiError::NotFound(ResourceType::Tag))?;
        api::verify_version(tag.last_edit_time, *client_version)?;

        let tag_id = tag.id;
        let tag_data = SnapshotData::retrieve(conn, tag)?;
        snapshot::tag::deletion_snapshot(conn, client, tag_data)?;

        diesel::delete(tag::table.find(tag_id)).execute(conn)?;
        Ok(Json(()))
    })
}

#[cfg(test)]
mod test {
    use crate::api::error::ApiResult;
    use crate::model::enums::ResourceType;
    use crate::model::tag::Tag;
    use crate::schema::{database_statistics, tag, tag_name, tag_statistics};
    use crate::search::tag::Token;
    use crate::string::SmallString;
    use crate::test::*;
    use crate::time::DateTime;
    use diesel::dsl::exists;
    use diesel::{ExpressionMethods, PgConnection, QueryDsl, QueryResult, RunQueryDsl, SelectableHelper};
    use serial_test::{parallel, serial};
    use strum::IntoEnumIterator;

    // Exclude fields that involve creation_time or last_edit_time
    const FIELDS: &str = "&fields=description,category,names,implications,suggestions,usages";

    #[tokio::test]
    #[parallel]
    async fn list() -> ApiResult<()> {
        const QUERY: &str = "GET /tags/?query";
        const PARAMS: &str = "-sort:name&limit=40&fields=names";
        verify_response(&format!("{QUERY}=-sort:name&limit=40{FIELDS}"), "tag/list").await?;

        let filter_table = crate::search::tag::filter_table();
        for token in Token::iter() {
            let filter = filter_table[token];
            let (sign, filter) = if filter.starts_with('-') {
                filter.split_at(1)
            } else {
                ("", filter)
            };
            let query = format!("{QUERY}={sign}{token}:{filter} {PARAMS}");
            let path = format!("tag/list_{token}_filtered");
            verify_response(&query, &path).await?;

            let query = format!("{QUERY}=sort:{token} {PARAMS}");
            let path = format!("tag/list_{token}_sorted");
            verify_response(&query, &path).await?;
        }
        Ok(())
    }

    #[tokio::test]
    #[parallel]
    async fn get() -> ApiResult<()> {
        const NAME: &str = "night_sky";
        let get_last_edit_time = |conn: &mut PgConnection| -> QueryResult<DateTime> {
            tag::table
                .select(tag::last_edit_time)
                .inner_join(tag_name::table)
                .filter(tag_name::name.eq(NAME))
                .first(conn)
        };

        let mut conn = get_connection()?;
        let last_edit_time = get_last_edit_time(&mut conn)?;

        verify_response(&format!("GET /tag/{NAME}/?{FIELDS}"), "tag/get").await?;

        let new_last_edit_time = get_last_edit_time(&mut conn)?;
        assert_eq!(new_last_edit_time, last_edit_time);
        Ok(())
    }

    #[tokio::test]
    #[parallel]
    async fn get_siblings() -> ApiResult<()> {
        const NAME: &str = "plant";
        let get_last_edit_time = |conn: &mut PgConnection| -> QueryResult<DateTime> {
            tag::table
                .select(tag::last_edit_time)
                .inner_join(tag_name::table)
                .filter(tag_name::name.eq(NAME))
                .first(conn)
        };

        let mut conn = get_connection()?;
        let last_edit_time = get_last_edit_time(&mut conn)?;

        verify_response(&format!("GET /tag-siblings/{NAME}/?{FIELDS}"), "tag/get_siblings").await?;

        let new_last_edit_time = get_last_edit_time(&mut conn)?;
        assert_eq!(new_last_edit_time, last_edit_time);
        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn create() -> ApiResult<()> {
        let get_tag_count = |conn: &mut PgConnection| -> QueryResult<i64> {
            database_statistics::table
                .select(database_statistics::tag_count)
                .first(conn)
        };

        let mut conn = get_connection()?;
        let tag_count = get_tag_count(&mut conn)?;

        verify_response(&format!("POST /tags/?{FIELDS}"), "tag/create").await?;

        let (tag_id, name): (i64, SmallString) = tag_name::table
            .select((tag_name::tag_id, tag_name::name))
            .order_by(tag_name::tag_id.desc())
            .first(&mut conn)?;

        let new_tag_count = get_tag_count(&mut conn)?;
        assert_eq!(new_tag_count, tag_count + 1);

        verify_response(&format!("DELETE /tag/{name}/?{FIELDS}"), "tag/delete").await?;

        let new_tag_count = get_tag_count(&mut conn)?;
        let has_tag: bool = diesel::select(exists(tag::table.find(tag_id))).first(&mut conn)?;
        assert_eq!(new_tag_count, tag_count);
        assert!(!has_tag);
        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn merge() -> ApiResult<()> {
        const REMOVE: &str = "stream";
        const MERGE_TO: &str = "night_sky";
        let get_tag_info = |conn: &mut PgConnection| -> QueryResult<(Tag, i64, i64, i64)> {
            tag::table
                .inner_join(tag_statistics::table)
                .inner_join(tag_name::table)
                .select((
                    Tag::as_select(),
                    tag_statistics::usage_count,
                    tag_statistics::implication_count,
                    tag_statistics::suggestion_count,
                ))
                .filter(tag_name::name.eq(MERGE_TO))
                .first(conn)
        };

        let mut conn = get_connection()?;
        let (tag, usage_count, implication_count, suggestion_count) = get_tag_info(&mut conn)?;
        let remove_id: i64 = tag_name::table
            .select(tag_name::tag_id)
            .filter(tag_name::name.eq(REMOVE))
            .first(&mut conn)?;

        verify_response(&format!("POST /tag-merge/?{FIELDS}"), "tag/merge").await?;

        let has_tag: bool = diesel::select(exists(tag::table.find(remove_id))).first(&mut conn)?;
        assert!(!has_tag);

        let (new_tag, new_usage_count, new_implication_count, new_suggestion_count) = get_tag_info(&mut conn)?;
        assert_eq!(new_tag.id, tag.id);
        assert_eq!(new_tag.category_id, tag.category_id);
        assert_eq!(new_tag.description, tag.description);
        assert_eq!(new_tag.creation_time, tag.creation_time);
        assert!(new_tag.last_edit_time > tag.last_edit_time);
        assert_ne!(new_usage_count, usage_count);
        assert_ne!(new_implication_count, implication_count);
        assert_ne!(new_suggestion_count, suggestion_count);
        reset_database();
        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn update() -> ApiResult<()> {
        const NAME: &str = "creek";
        let get_tag_info = |conn: &mut PgConnection, name: &str| -> QueryResult<(Tag, i64, i64, i64)> {
            tag::table
                .inner_join(tag_statistics::table)
                .inner_join(tag_name::table)
                .select((
                    Tag::as_select(),
                    tag_statistics::usage_count,
                    tag_statistics::implication_count,
                    tag_statistics::suggestion_count,
                ))
                .filter(tag_name::name.eq(name))
                .first(conn)
        };

        let mut conn = get_connection()?;
        let (tag, usage_count, implication_count, suggestion_count) = get_tag_info(&mut conn, NAME)?;

        verify_response(&format!("PUT /tag/{NAME}/?{FIELDS}"), "tag/edit").await?;

        let new_name: SmallString = tag_name::table
            .select(tag_name::name)
            .filter(tag_name::tag_id.eq(tag.id))
            .first(&mut conn)?;

        let (new_tag, new_usage_count, new_implication_count, new_suggestion_count) =
            get_tag_info(&mut conn, &new_name)?;
        assert_eq!(new_tag.id, tag.id);
        assert_ne!(new_tag.category_id, tag.category_id);
        assert_ne!(new_tag.description, tag.description);
        assert_eq!(new_tag.creation_time, tag.creation_time);
        assert!(new_tag.last_edit_time > tag.last_edit_time);
        assert_eq!(new_usage_count, usage_count);
        assert_ne!(new_implication_count, implication_count);
        assert_ne!(new_suggestion_count, suggestion_count);

        verify_response(&format!("PUT /tag/{new_name}/?{FIELDS}"), "tag/edit_restore").await?;

        let new_tag_id: i64 = tag::table.select(tag::id).order_by(tag::id.desc()).first(&mut conn)?;
        diesel::delete(tag::table.find(new_tag_id)).execute(&mut conn)?;

        let (new_tag, new_usage_count, new_implication_count, new_suggestion_count) = get_tag_info(&mut conn, NAME)?;
        assert_eq!(new_tag.id, tag.id);
        assert_eq!(new_tag.category_id, tag.category_id);
        assert_eq!(new_tag.description, tag.description);
        assert_eq!(new_tag.creation_time, tag.creation_time);
        assert!(new_tag.last_edit_time > tag.last_edit_time);
        assert_eq!(new_usage_count, usage_count);
        assert_eq!(new_implication_count, implication_count);
        assert_eq!(new_suggestion_count, suggestion_count);
        Ok(())
    }

    #[tokio::test]
    #[parallel]
    async fn error() -> ApiResult<()> {
        verify_response("GET /tag/none", "tag/get_nonexistent").await?;
        verify_response("GET /tag-siblings/none", "tag/get_siblings_of_nonexistent").await?;
        verify_response("POST /tag-merge", "tag/merge_to_nonexistent").await?;
        verify_response("POST /tag-merge", "tag/merge_with_nonexistent").await?;
        verify_response("PUT /tag/none", "tag/edit_nonexistent").await?;
        verify_response("DELETE /tag/none", "tag/delete_nonexistent").await?;

        verify_response("POST /tags", "tag/create_nameless").await?;
        verify_response("POST /tags", "tag/create_name_clash").await?;
        verify_response("POST /tags", "tag/create_invalid_name").await?;
        verify_response("POST /tags", "tag/create_invalid_category").await?;
        verify_response("POST /tags", "tag/create_invalid_suggestion").await?;
        verify_response("POST /tags", "tag/create_invalid_implication").await?;
        verify_response("POST /tag-merge", "tag/self-merge").await?;

        verify_response("PUT /tag/sky", "tag/edit_nameless").await?;
        verify_response("PUT /tag/sky", "tag/edit_name_clash").await?;
        verify_response("PUT /tag/sky", "tag/edit_invalid_name").await?;
        verify_response("PUT /tag/sky", "tag/edit_invalid_category").await?;
        verify_response("PUT /tag/sky", "tag/edit_invalid_suggestion").await?;
        verify_response("PUT /tag/sky", "tag/edit_invalid_implication").await?;
        verify_response("PUT /tag/plant", "tag/edit_cyclic_implication").await?;

        reset_sequence(ResourceType::Tag)?;
        Ok(())
    }
}
