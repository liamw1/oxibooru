use crate::api::{ApiResult, DeleteBody, MergeBody, PageParams, PagedResponse, ResourceParams};
use crate::auth::Client;
use crate::model::enums::ResourceType;
use crate::model::post::PostTag;
use crate::model::tag::{NewTag, TagImplication, TagSuggestion};
use crate::resource::tag::TagInfo;
use crate::schema::{post_tag, tag, tag_category, tag_implication, tag_name, tag_suggestion};
use crate::search::tag::QueryBuilder;
use crate::string::SmallString;
use crate::time::DateTime;
use crate::{api, config, db, resource, update};
use axum::extract::{Extension, Path, Query};
use axum::{Json, Router, routing};
use diesel::dsl::{count_star, max};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub fn routes() -> Router {
    Router::new()
        .route("/tags", routing::get(list).post(create))
        .route("/tag/{name}", routing::get(get).put(update).delete(delete))
        .route("/tag-siblings/{name}", routing::get(get_siblings))
        .route("/tag-merge", routing::post(merge))
}

const MAX_TAGS_PER_PAGE: i64 = 1000;
const MAX_TAG_SIBLINGS: i64 = 1000;

async fn list(
    Extension(client): Extension<Client>,
    Query(params): Query<PageParams>,
) -> ApiResult<Json<PagedResponse<TagInfo>>> {
    params.bump_login(client)?;
    api::verify_privilege(client, config::privileges().tag_list)?;

    let offset = params.offset.unwrap_or(0);
    let limit = std::cmp::min(params.limit.get(), MAX_TAGS_PER_PAGE);
    let fields = resource::create_table(params.fields()).map_err(Box::from)?;

    db::get_connection()?.transaction(|conn| {
        let mut query_builder = QueryBuilder::new(params.criteria())?;
        query_builder.set_offset_and_limit(offset, limit);

        let total = query_builder.count(conn)?;
        let selected_tags = query_builder.load(conn)?;
        Ok(Json(PagedResponse {
            query: params.into_query(),
            offset,
            limit,
            total,
            results: TagInfo::new_batch_from_ids(conn, selected_tags, &fields)?,
        }))
    })
}

async fn get(
    Extension(client): Extension<Client>,
    Path(name): Path<String>,
    Query(params): Query<ResourceParams>,
) -> ApiResult<Json<TagInfo>> {
    params.bump_login(client)?;
    api::verify_privilege(client, config::privileges().tag_view)?;

    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    let name = percent_encoding::percent_decode_str(&name).decode_utf8()?;
    db::get_connection()?.transaction(|conn| {
        let tag_id = tag_name::table
            .select(tag_name::tag_id)
            .filter(tag_name::name.eq(name))
            .first(conn)
            .optional()?
            .ok_or(api::Error::NotFound(ResourceType::Tag))?;
        TagInfo::new_from_id(conn, tag_id, &fields)
            .map(Json)
            .map_err(api::Error::from)
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

async fn get_siblings(
    Extension(client): Extension<Client>,
    Path(name): Path<String>,
    Query(params): Query<ResourceParams>,
) -> ApiResult<Json<TagSiblings>> {
    params.bump_login(client)?;
    api::verify_privilege(client, config::privileges().tag_view)?;

    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    let name = percent_encoding::percent_decode_str(&name).decode_utf8()?;
    db::get_connection()?.transaction(|conn| {
        let tag_id: i64 = tag::table
            .select(tag::id)
            .inner_join(tag_name::table)
            .filter(tag_name::name.eq(name))
            .first(conn)?;
        let posts_tagged_on = post_tag::table
            .select(post_tag::post_id)
            .filter(post_tag::tag_id.eq(tag_id))
            .into_boxed();
        let (sibling_ids, common_post_counts): (_, Vec<_>) = post_tag::table
            .group_by(post_tag::tag_id)
            .select((post_tag::tag_id, count_star()))
            .filter(post_tag::post_id.eq_any(posts_tagged_on))
            .filter(post_tag::tag_id.ne(tag_id))
            .order_by((count_star().desc(), post_tag::tag_id))
            .limit(MAX_TAG_SIBLINGS)
            .load::<(i64, i64)>(conn)?
            .into_iter()
            .unzip();

        let siblings = TagInfo::new_batch_from_ids(conn, sibling_ids, &fields)?
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
    category: String,
    description: Option<String>,
    names: Vec<SmallString>,
    implications: Option<Vec<SmallString>>,
    suggestions: Option<Vec<SmallString>>,
}

async fn create(
    Extension(client): Extension<Client>,
    Query(params): Query<ResourceParams>,
    Json(body): Json<CreateBody>,
) -> ApiResult<Json<TagInfo>> {
    params.bump_login(client)?;
    api::verify_privilege(client, config::privileges().tag_create)?;

    if body.names.is_empty() {
        return Err(api::Error::NoNamesGiven(ResourceType::Tag));
    }

    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    let mut conn = db::get_connection()?;
    let tag_id = conn.transaction(|conn| {
        let category_id: i64 = tag_category::table
            .select(tag_category::id)
            .filter(tag_category::name.eq(body.category))
            .first(conn)?;
        let new_tag = NewTag {
            category_id,
            description: body.description.as_deref().unwrap_or(""),
        };
        let tag_id: i64 = diesel::insert_into(tag::table)
            .values(new_tag)
            .returning(tag::id)
            .get_result(conn)?;

        update::tag::add_names(conn, tag_id, 0, body.names)?;
        if let Some(implications) = body.implications {
            let implied_ids = update::tag::get_or_create_tag_ids(conn, client, &implications, true)?;
            update::tag::add_implications(conn, tag_id, implied_ids)?;
        }
        if let Some(suggestions) = body.suggestions {
            let suggested_ids = update::tag::get_or_create_tag_ids(conn, client, &suggestions, true)?;
            update::tag::add_suggestions(conn, tag_id, suggested_ids)?;
        }
        Ok::<_, api::Error>(tag_id)
    })?;
    conn.transaction(|conn| TagInfo::new_from_id(conn, tag_id, &fields))
        .map(Json)
        .map_err(api::Error::from)
}

async fn merge(
    Extension(client): Extension<Client>,
    Query(params): Query<ResourceParams>,
    Json(body): Json<MergeBody<String>>,
) -> ApiResult<Json<TagInfo>> {
    params.bump_login(client)?;
    api::verify_privilege(client, config::privileges().tag_merge)?;

    let get_tag_info = |conn: &mut PgConnection, name: String| {
        tag::table
            .select((tag::id, tag::last_edit_time))
            .inner_join(tag_name::table)
            .filter(tag_name::name.eq(name))
            .first(conn)
    };

    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    let mut conn = db::get_connection()?;
    let merged_tag_id = conn.transaction(|conn| {
        let (remove_id, remove_version) = get_tag_info(conn, body.remove)?;
        let (merge_to_id, merge_to_version) = get_tag_info(conn, body.merge_to)?;
        if remove_id == merge_to_id {
            return Err(api::Error::SelfMerge(ResourceType::Tag));
        }
        api::verify_version(remove_version, body.remove_version)?;
        api::verify_version(merge_to_version, body.merge_to_version)?;

        // Merge implications
        let involved_implications: Vec<TagImplication> = tag_implication::table
            .filter(tag_implication::parent_id.eq(remove_id))
            .or_filter(tag_implication::child_id.eq(remove_id))
            .or_filter(tag_implication::parent_id.eq(merge_to_id))
            .or_filter(tag_implication::child_id.eq(merge_to_id))
            .load(conn)?;
        let merged_implications: HashSet<_> = involved_implications
            .iter()
            .copied()
            .map(|mut implication| {
                if implication.parent_id == remove_id {
                    implication.parent_id = merge_to_id
                } else if implication.child_id == remove_id {
                    implication.child_id = merge_to_id
                }
                implication
            })
            .filter(|implication| implication.parent_id != implication.child_id)
            .collect();
        diesel::delete(tag_implication::table)
            .filter(tag_implication::parent_id.eq(merge_to_id))
            .or_filter(tag_implication::child_id.eq(merge_to_id))
            .execute(conn)?;
        diesel::insert_into(tag_implication::table)
            .values(merged_implications.into_iter().collect::<Vec<_>>())
            .execute(conn)?;

        // Merge suggestions
        let involved_suggestions: Vec<TagSuggestion> = tag_suggestion::table
            .filter(tag_suggestion::parent_id.eq(remove_id))
            .or_filter(tag_suggestion::child_id.eq(remove_id))
            .or_filter(tag_suggestion::parent_id.eq(merge_to_id))
            .or_filter(tag_suggestion::child_id.eq(merge_to_id))
            .load(conn)?;
        let merged_suggestions: HashSet<_> = involved_suggestions
            .iter()
            .copied()
            .map(|mut suggestion| {
                if suggestion.parent_id == remove_id {
                    suggestion.parent_id = merge_to_id
                } else if suggestion.child_id == remove_id {
                    suggestion.child_id = merge_to_id
                }
                suggestion
            })
            .filter(|suggestion| suggestion.parent_id != suggestion.child_id)
            .collect();
        diesel::delete(tag_suggestion::table)
            .filter(tag_suggestion::parent_id.eq(merge_to_id))
            .or_filter(tag_suggestion::child_id.eq(merge_to_id))
            .execute(conn)?;
        diesel::insert_into(tag_suggestion::table)
            .values(merged_suggestions.into_iter().collect::<Vec<_>>())
            .execute(conn)?;

        // Merge usages
        let merge_to_posts = post_tag::table
            .select(post_tag::post_id)
            .filter(post_tag::tag_id.eq(merge_to_id))
            .into_boxed();
        let new_post_tags: Vec<_> = post_tag::table
            .select(post_tag::post_id)
            .filter(post_tag::tag_id.eq(remove_id))
            .filter(post_tag::post_id.ne_all(merge_to_posts))
            .load(conn)?
            .into_iter()
            .map(|post_id| PostTag {
                post_id,
                tag_id: merge_to_id,
            })
            .collect();
        diesel::insert_into(post_tag::table)
            .values(new_post_tags)
            .execute(conn)?;

        // Merge names
        let current_name_count = tag_name::table
            .select(max(tag_name::order) + 1)
            .filter(tag_name::tag_id.eq(merge_to_id))
            .first::<Option<_>>(conn)?
            .unwrap_or(0);
        let removed_names = diesel::delete(tag_name::table.filter(tag_name::tag_id.eq(remove_id)))
            .returning(tag_name::name)
            .get_results(conn)?;
        update::tag::add_names(conn, merge_to_id, current_name_count, removed_names)?;

        diesel::delete(tag::table.find(remove_id)).execute(conn)?;
        update::tag::last_edit_time(conn, merge_to_id).map(|_| merge_to_id)
    })?;
    conn.transaction(|conn| TagInfo::new_from_id(conn, merged_tag_id, &fields))
        .map(Json)
        .map_err(api::Error::from)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdateBody {
    version: DateTime,
    category: Option<SmallString>,
    description: Option<String>,
    names: Option<Vec<SmallString>>,
    implications: Option<Vec<SmallString>>,
    suggestions: Option<Vec<SmallString>>,
}

async fn update(
    Extension(client): Extension<Client>,
    Path(name): Path<String>,
    Query(params): Query<ResourceParams>,
    Json(body): Json<UpdateBody>,
) -> ApiResult<Json<TagInfo>> {
    params.bump_login(client)?;

    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    let name = percent_encoding::percent_decode_str(&name).decode_utf8()?;
    let mut conn = db::get_connection()?;
    let tag_id = conn.transaction(|conn| {
        let (tag_id, tag_version) = tag::table
            .select((tag::id, tag::last_edit_time))
            .inner_join(tag_name::table)
            .filter(tag_name::name.eq(name))
            .first(conn)?;
        api::verify_version(tag_version, body.version)?;

        if let Some(category) = body.category {
            api::verify_privilege(client, config::privileges().tag_edit_category)?;

            let category_id: i64 = tag_category::table
                .select(tag_category::id)
                .filter(tag_category::name.eq(category))
                .first(conn)?;
            diesel::update(tag::table.find(tag_id))
                .set(tag::category_id.eq(category_id))
                .execute(conn)?;
        }
        if let Some(description) = body.description {
            api::verify_privilege(client, config::privileges().tag_edit_description)?;

            diesel::update(tag::table.find(tag_id))
                .set(tag::description.eq(description))
                .execute(conn)?;
        }
        if let Some(names) = body.names {
            api::verify_privilege(client, config::privileges().tag_edit_name)?;
            if names.is_empty() {
                return Err(api::Error::NoNamesGiven(ResourceType::Tag));
            }

            update::tag::delete_names(conn, tag_id)?;
            update::tag::add_names(conn, tag_id, 0, names)?;
        }
        if let Some(implications) = body.implications {
            api::verify_privilege(client, config::privileges().tag_edit_implication)?;

            let implied_ids = update::tag::get_or_create_tag_ids(conn, client, &implications, true)?;
            diesel::delete(tag_implication::table)
                .filter(tag_implication::parent_id.eq(tag_id))
                .execute(conn)?;
            update::tag::add_implications(conn, tag_id, implied_ids)?;
        }
        if let Some(suggestions) = body.suggestions {
            api::verify_privilege(client, config::privileges().tag_edit_suggestion)?;

            let suggested_ids = update::tag::get_or_create_tag_ids(conn, client, &suggestions, true)?;
            diesel::delete(tag_suggestion::table)
                .filter(tag_suggestion::parent_id.eq(tag_id))
                .execute(conn)?;
            update::tag::add_suggestions(conn, tag_id, suggested_ids)?;
        }
        update::tag::last_edit_time(conn, tag_id).map(|_| tag_id)
    })?;
    conn.transaction(|conn| TagInfo::new_from_id(conn, tag_id, &fields))
        .map(Json)
        .map_err(api::Error::from)
}

async fn delete(
    Extension(client): Extension<Client>,
    Path(name): Path<String>,
    Json(client_version): Json<DeleteBody>,
) -> ApiResult<Json<()>> {
    api::verify_privilege(client, config::privileges().tag_delete)?;

    let name = percent_encoding::percent_decode_str(&name).decode_utf8()?;
    db::get_connection()?.transaction(|conn| {
        let (tag_id, tag_version): (i64, DateTime) = tag::table
            .select((tag::id, tag::last_edit_time))
            .inner_join(tag_name::table)
            .filter(tag_name::name.eq(name))
            .first(conn)?;
        api::verify_version(tag_version, *client_version)?;

        diesel::delete(tag::table.find(tag_id)).execute(conn)?;
        Ok(Json(()))
    })
}

#[cfg(test)]
mod test {
    use crate::api::ApiResult;
    use crate::model::tag::Tag;
    use crate::schema::{database_statistics, tag, tag_name, tag_statistics};
    use crate::test::*;
    use crate::time::DateTime;
    use diesel::dsl::exists;
    use diesel::prelude::*;
    use serial_test::{parallel, serial};

    // Exclude fields that involve creation_time or last_edit_time
    const FIELDS: &str = "&fields=description,category,names,implications,suggestions,usages";

    #[tokio::test]
    #[parallel]
    async fn list() -> ApiResult<()> {
        const QUERY: &str = "GET /tags/?query";
        const SORT: &str = "-sort:name&limit=40";
        verify_query(&format!("{QUERY}={SORT}{FIELDS}"), "tag/list.json").await?;
        verify_query(&format!("{QUERY}=sort:usage-count -sort:name&limit=1{FIELDS}"), "tag/list_most_used.json")
            .await?;
        verify_query(&format!("{QUERY}=category:Character {SORT}{FIELDS}"), "tag/list_category_character.json")
            .await?;
        verify_query(&format!("{QUERY}=*sky* {SORT}{FIELDS}"), "tag/list_has_sky_in_name.json").await?;
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

        verify_query(&format!("GET /tag/{NAME}/?{FIELDS}"), "tag/get.json").await?;

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

        verify_query(&format!("GET /tag-siblings/{NAME}/?{FIELDS}"), "tag/get_siblings.json").await?;

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

        verify_query(&format!("POST /tags/?{FIELDS}"), "tag/create.json").await?;

        let (tag_id, name): (i64, String) = tag_name::table
            .select((tag_name::tag_id, tag_name::name))
            .order_by(tag_name::tag_id.desc())
            .first(&mut conn)?;

        let new_tag_count = get_tag_count(&mut conn)?;
        assert_eq!(new_tag_count, tag_count + 1);

        verify_query(&format!("DELETE /tag/{name}/?{FIELDS}"), "delete.json").await?;

        let new_tag_count = get_tag_count(&mut conn)?;
        let has_tag: bool = diesel::select(exists(tag::table.find(tag_id))).get_result(&mut conn)?;
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

        verify_query(&format!("POST /tag-merge/?{FIELDS}"), "tag/merge.json").await?;

        let has_tag: bool = diesel::select(exists(tag::table.find(remove_id))).get_result(&mut conn)?;
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
        Ok(reset_database())
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

        verify_query(&format!("PUT /tag/{NAME}/?{FIELDS}"), "tag/update.json").await?;

        let new_name: String = tag_name::table
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

        verify_query(&format!("PUT /tag/{new_name}/?{FIELDS}"), "tag/update_restore.json").await?;

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
}
