use crate::auth::Client;
use crate::config::Config;
use crate::model::enums::UserRank;
use crate::model::tag_category::TagCategory;
use crate::resource::BoolFill;
use crate::schema::{tag_category, tag_category_statistics};
use crate::string::SmallString;
use crate::time::DateTime;
use diesel::{ExpressionMethods, PgConnection, QueryDsl, QueryResult, RunQueryDsl, SelectableHelper};
use serde::Serialize;
use serde_with::skip_serializing_none;
use server_macros::non_nullable_options;
use strum::{EnumString, EnumTable};
use utoipa::ToSchema;

#[derive(Clone, Copy, EnumString, EnumTable)]
#[strum(serialize_all = "camelCase")]
pub enum Field {
    Version,
    Name,
    Color,
    Usages,
    Order,
    Default,
}

impl BoolFill for FieldTable<bool> {
    fn filled(val: bool) -> Self {
        Self::filled(val)
    }
}

/// A single tag category. The primary purpose of tag categories is to distinguish
/// certain tag types (such as characters, media type etc.), which improves user
/// experience.
#[non_nullable_options]
#[skip_serializing_none]
#[derive(Serialize, ToSchema)]
pub struct TagCategoryInfo {
    /// Resource version. See [versioning](#Versioning).
    version: Option<DateTime>,
    /// The category name.
    name: Option<SmallString>,
    /// The category color.
    color: Option<SmallString>,
    /// How many tags is the given category used with.
    usages: Option<i64>,
    /// The order in which tags with this category are displayed, ascending.
    order: Option<i32>,
    /// Whether the tag category is the default one.
    default: Option<bool>,
}

impl TagCategoryInfo {
    pub fn new(conn: &mut PgConnection, category: TagCategory, fields: &FieldTable<bool>) -> QueryResult<Self> {
        let usages = tag_category_statistics::table
            .find(category.id)
            .select(tag_category_statistics::usage_count)
            .first(conn)?;
        Ok(Self {
            version: fields[Field::Version].then_some(category.last_edit_time),
            name: fields[Field::Name].then_some(category.name),
            color: fields[Field::Color].then_some(category.color),
            usages: fields[Field::Usages].then_some(usages),
            order: fields[Field::Order].then_some(category.order),
            default: fields[Field::Default].then_some(category.id == 0),
        })
    }

    pub fn all(
        conn: &mut PgConnection,
        config: &Config,
        client: Client,
        fields: &FieldTable<bool>,
    ) -> QueryResult<Vec<Self>> {
        let mut tag_categories = tag_category::table
            .inner_join(tag_category_statistics::table)
            .select((TagCategory::as_select(), tag_category_statistics::usage_count))
            .order(tag_category::order)
            .into_boxed();
        if client.rank == UserRank::Anonymous {
            tag_categories =
                tag_categories.filter(tag_category::name.ne_all(&config.anonymous_preferences.tag_category_blacklist));
        }

        let tag_categories: Vec<(TagCategory, i64)> = tag_categories.load(conn)?;
        Ok(tag_categories
            .into_iter()
            .map(|(category, usages)| Self {
                version: fields[Field::Version].then_some(category.last_edit_time),
                name: fields[Field::Name].then_some(category.name),
                color: fields[Field::Color].then_some(category.color),
                usages: fields[Field::Usages].then_some(usages),
                order: fields[Field::Order].then_some(category.order),
                default: fields[Field::Default].then_some(category.id == 0),
            })
            .collect())
    }
}
