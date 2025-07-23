use crate::api::{ApiResult, DeleteBody, MergeBody, PageParams, PagedResponse, ResourceParams};
use crate::auth::Client;
use crate::model::enums::ResourceType;
use crate::model::tag::{NewTag, Tag};
use crate::resource::tag::TagInfo;
use crate::schema::{post_tag, tag, tag_category, tag_implication, tag_name, tag_suggestion};
use crate::search::tag::QueryBuilder;
use crate::snapshot::tag::SnapshotData;
use crate::string::SmallString;
use crate::time::DateTime;
use crate::{api, config, db, resource, snapshot, update};
use axum::extract::{Extension, Path, Query};
use axum::{Json, Router, routing};
use diesel::dsl::count_star;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

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
    api::verify_privilege(client, config::privileges().tag_view)?;

    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
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
    api::verify_privilege(client, config::privileges().tag_view)?;

    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
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
    api::verify_privilege(client, config::privileges().tag_create)?;

    if body.names.is_empty() {
        return Err(api::Error::NoNamesGiven(ResourceType::Tag));
    }

    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    let mut conn = db::get_connection()?;
    let tag = conn.transaction(|conn| {
        let (category_id, category): (i64, SmallString) = tag_category::table
            .select((tag_category::id, tag_category::name))
            .filter(tag_category::name.eq(body.category))
            .first(conn)?;
        let tag: Tag = NewTag {
            category_id,
            description: body.description.as_deref().unwrap_or(""),
        }
        .insert_into(tag::table)
        .get_result(conn)?;

        // Add names, implications, and suggestions
        update::tag::add_names(conn, tag.id, 0, &body.names)?;
        let (implied_ids, implications) =
            update::tag::get_or_create_tag_ids(conn, client, body.implications.as_deref().unwrap_or_default(), true)?;
        let (suggested_ids, suggestions) =
            update::tag::get_or_create_tag_ids(conn, client, body.suggestions.as_deref().unwrap_or_default(), true)?;
        update::tag::add_implications(conn, tag.id, &implied_ids)?;
        update::tag::add_suggestions(conn, tag.id, &suggested_ids)?;

        let tag_data = SnapshotData {
            category,
            names: body.names,
            implications,
            suggestions,
        };
        snapshot::tag::creation_snapshot(conn, client, tag_data)?;
        Ok::<_, api::Error>(tag)
    })?;
    conn.transaction(|conn| TagInfo::new(conn, tag, &fields))
        .map(Json)
        .map_err(api::Error::from)
}

async fn merge(
    Extension(client): Extension<Client>,
    Query(params): Query<ResourceParams>,
    Json(body): Json<MergeBody<SmallString>>,
) -> ApiResult<Json<TagInfo>> {
    api::verify_privilege(client, config::privileges().tag_merge)?;

    let get_tag_info = |conn: &mut PgConnection, name: &str| {
        tag::table
            .select((tag::id, tag::last_edit_time))
            .inner_join(tag_name::table)
            .filter(tag_name::name.eq(name))
            .first(conn)
    };

    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    let mut conn = db::get_connection()?;
    let merged_tag_id = conn.transaction(|conn| {
        let (absorbed_id, absorbed_version) = get_tag_info(conn, &body.remove)?;
        let (merge_to_id, merge_to_version) = get_tag_info(conn, &body.merge_to)?;
        if absorbed_id == merge_to_id {
            return Err(api::Error::SelfMerge(ResourceType::Tag));
        }
        api::verify_version(absorbed_version, body.remove_version)?;
        api::verify_version(merge_to_version, body.merge_to_version)?;

        update::tag::merge(conn, absorbed_id, merge_to_id)?;
        snapshot::tag::merge_snapshot(conn, client, body.remove, body.merge_to)?;
        Ok::<_, api::Error>(merge_to_id)
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
    let fields = resource::create_table(params.fields()).map_err(Box::from)?;

    let mut conn = db::get_connection()?;
    let tag_id = conn.transaction(|conn| {
        let old_tag: Tag = tag::table
            .inner_join(tag_name::table)
            .select(Tag::as_select())
            .filter(tag_name::name.eq(name))
            .first(conn)?;
        api::verify_version(old_tag.last_edit_time, body.version)?;

        let mut new_tag = old_tag.clone();
        let old_snapshot_data = SnapshotData::retrieve(conn, old_tag.id, old_tag.category_id)?;
        let mut new_snapshot_data = old_snapshot_data.clone();

        if let Some(category) = body.category {
            api::verify_privilege(client, config::privileges().tag_edit_category)?;

            let category_id: i64 = tag_category::table
                .select(tag_category::id)
                .filter(tag_category::name.eq(&category))
                .first(conn)?;
            new_tag.category_id = category_id;
            new_snapshot_data.category = category;
        }
        if let Some(description) = body.description {
            api::verify_privilege(client, config::privileges().tag_edit_description)?;
            new_tag.description = description;
        }
        if let Some(names) = body.names {
            api::verify_privilege(client, config::privileges().tag_edit_name)?;
            if names.is_empty() {
                return Err(api::Error::NoNamesGiven(ResourceType::Tag));
            }

            update::tag::delete_names(conn, old_tag.id)?;
            update::tag::add_names(conn, old_tag.id, 0, &names)?;
            new_snapshot_data.names = names;
        }
        if let Some(implications) = body.implications {
            api::verify_privilege(client, config::privileges().tag_edit_implication)?;

            let (implied_ids, implications) = update::tag::get_or_create_tag_ids(conn, client, &implications, true)?;
            diesel::delete(tag_implication::table)
                .filter(tag_implication::parent_id.eq(old_tag.id))
                .execute(conn)?;
            update::tag::add_implications(conn, old_tag.id, &implied_ids)?;
            new_snapshot_data.implications = implications;
        }
        if let Some(suggestions) = body.suggestions {
            api::verify_privilege(client, config::privileges().tag_edit_suggestion)?;

            let (suggested_ids, suggestions) = update::tag::get_or_create_tag_ids(conn, client, &suggestions, true)?;
            diesel::delete(tag_suggestion::table)
                .filter(tag_suggestion::parent_id.eq(old_tag.id))
                .execute(conn)?;
            update::tag::add_suggestions(conn, old_tag.id, &suggested_ids)?;
            new_snapshot_data.suggestions = suggestions;
        }

        new_tag.last_edit_time = DateTime::now();
        let _: Tag = new_tag.save_changes(conn)?;
        snapshot::tag::modification_snapshot(conn, client, old_snapshot_data, new_snapshot_data)?;
        Ok::<_, api::Error>(old_tag.id)
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

    db::get_connection()?.transaction(|conn| {
        let (tag_id, category_id, tag_version): (i64, i64, DateTime) = tag::table
            .select((tag::id, tag::category_id, tag::last_edit_time))
            .inner_join(tag_name::table)
            .filter(tag_name::name.eq(name))
            .first(conn)?;
        api::verify_version(tag_version, *client_version)?;

        let tag_data = SnapshotData::retrieve(conn, tag_id, category_id)?;
        snapshot::tag::deletion_snapshot(conn, client, tag_data)?;

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
        verify_query(&format!("{QUERY}=category:Character {SORT}{FIELDS}"), "tag/list_category_character.json").await?;
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
