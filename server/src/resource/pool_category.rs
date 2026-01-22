use crate::model::pool_category::PoolCategory;
use crate::resource::BoolFill;
use crate::schema::{pool_category, pool_category_statistics};
use crate::string::SmallString;
use crate::time::DateTime;
use diesel::{PgConnection, QueryDsl, QueryResult, RunQueryDsl, SelectableHelper};
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
    Default,
}

impl BoolFill for FieldTable<bool> {
    fn filled(val: bool) -> Self {
        Self::filled(val)
    }
}

/// A single pool category. The primary purpose of pool categories is to distinguish
/// certain pool types (such as series, relations etc.), which improves user
/// experience.
#[non_nullable_options]
#[skip_serializing_none]
#[derive(Serialize, ToSchema)]
pub struct PoolCategoryInfo {
    /// Resource version. See [versioning](#Versioning).
    version: Option<DateTime>,
    /// The category name.
    name: Option<SmallString>,
    /// The category color.
    color: Option<SmallString>,
    /// How many pools is the given category used with.
    usages: Option<i64>,
    /// Whether the pool category is the default one.
    default: Option<bool>,
}

impl PoolCategoryInfo {
    pub fn new(conn: &mut PgConnection, category: PoolCategory, fields: &FieldTable<bool>) -> QueryResult<Self> {
        let usages = pool_category_statistics::table
            .find(category.id)
            .select(pool_category_statistics::usage_count)
            .first(conn)?;
        Ok(Self {
            version: fields[Field::Version].then_some(category.last_edit_time),
            name: fields[Field::Name].then_some(category.name),
            color: fields[Field::Color].then_some(category.color),
            usages: fields[Field::Usages].then_some(usages),
            default: fields[Field::Default].then_some(category.id == 0),
        })
    }

    pub fn all(conn: &mut PgConnection, fields: &FieldTable<bool>) -> QueryResult<Vec<Self>> {
        let pool_categories: Vec<(PoolCategory, i64)> = pool_category::table
            .inner_join(pool_category_statistics::table)
            .select((PoolCategory::as_select(), pool_category_statistics::usage_count))
            .order_by(pool_category::id)
            .load(conn)?;
        Ok(pool_categories
            .into_iter()
            .map(|(category, usages)| Self {
                version: fields[Field::Version].then_some(category.last_edit_time),
                name: fields[Field::Name].then_some(category.name),
                color: fields[Field::Color].then_some(category.color),
                usages: fields[Field::Usages].then_some(usages),
                default: fields[Field::Default].then_some(category.id == 0),
            })
            .collect())
    }
}
