use crate::model::pool_category::PoolCategory;
use crate::resource::field::Mask;
use crate::schema::{pool_category, pool_category_statistics};
use crate::string::SmallString;
use crate::time::DateTime;
use diesel::{PgConnection, QueryDsl, QueryResult, RunQueryDsl, SelectableHelper};
use serde::Serialize;
use server_macros::resource;
use strum::EnumString;
use utoipa::ToSchema;

#[derive(Clone, Copy, EnumString)]
#[strum(serialize_all = "camelCase")]
pub enum Field {
    Version,
    Name,
    Color,
    Usages,
    Default,
}

impl From<Field> for u64 {
    fn from(value: Field) -> Self {
        value as u64
    }
}

/// A single pool category. The primary purpose of pool categories is to distinguish
/// certain pool types (such as series, relations etc.), which improves user
/// experience.
#[resource]
#[derive(Serialize, ToSchema)]
pub struct PoolCategoryInfo {
    /// Resource version. See [versioning](#Versioning).
    version: DateTime,
    /// The category name.
    name: SmallString,
    /// The category color.
    color: SmallString,
    /// How many pools is the given category used with.
    usages: i64,
    /// Whether the pool category is the default one.
    default: bool,
}

impl PoolCategoryInfo {
    pub fn new(conn: &mut PgConnection, category: PoolCategory, fields: Mask<Field>) -> QueryResult<Self> {
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

    pub fn all(conn: &mut PgConnection, fields: Mask<Field>) -> QueryResult<Vec<Self>> {
        let pool_categories: Vec<(PoolCategory, i64)> = pool_category::table
            .inner_join(pool_category_statistics::table)
            .select((PoolCategory::as_select(), pool_category_statistics::usage_count))
            .order(pool_category::id)
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
