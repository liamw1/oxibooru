use crate::api::error::{ApiError, ApiResult};
use crate::api::pool::PoolUpdateBody;
use crate::extract::{Ctx, DeleteBody};
use crate::model::pool_category::PoolCategory;
use crate::resource::pool::{Field, MicroPool, PoolInfo};
use crate::resource::{JoinExt, NotRequested};
use crate::schema::pool_category;
use crate::string::{LargeString, SmallString};
use crate::time::DateTime;
use crate::web::form::{self, ElementClass, FormField};
use crate::web::{PathForm, WebResult};
use crate::{string, update, web};
use diesel::{QueryDsl, RunQueryDsl};
use serde::Deserialize;
use std::collections::{BTreeMap, HashSet};
use std::convert::Infallible;
use std::ops::{Deref, DerefMut};

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Element {
    primary_name: SmallString,
    pub category: SmallString,
    pub post_count: i64,
    #[serde(skip)]
    pub class: ElementClass,
}

impl Element {
    pub fn from_micropool(pool: MicroPool, class: ElementClass) -> Self {
        Self {
            primary_name: pool.names[0].clone(),
            category: pool.category,
            post_count: pool.post_count,
            class,
        }
    }

    pub fn primary_name(&self) -> &str {
        &self.primary_name
    }

    pub fn url(&self) -> String {
        web::pool_url(&self.primary_name)
    }

    pub fn search_url(&self) -> String {
        web::pool_search_url(&self.primary_name)
    }

    pub fn class(&self) -> ElementClass {
        self.class
    }
}

impl From<MicroPool> for Element {
    fn from(pool: MicroPool) -> Self {
        Self::from_micropool(pool, ElementClass::None)
    }
}

impl PartialEq for Element {
    fn eq(&self, other: &Self) -> bool {
        self.primary_name == other.primary_name
    }
}

impl Eq for Element {}

#[derive(Default, PartialEq, Eq, Deserialize)]
pub struct ElementMap(BTreeMap<i64, Element>);

impl ElementMap {
    pub fn names(&self) -> Vec<SmallString> {
        self.values().map(|element| element.primary_name.clone()).collect()
    }

    pub async fn append_pools(&mut self, Ctx(ctx, connection_pool): Ctx, joined_names: &str) -> WebResult<()> {
        const FIELDS: [Field; 3] = [Field::Category, Field::Names, Field::PostCount];

        let input_names: HashSet<_> = string::split_unescaped_whitespace(joined_names).collect();
        let input_names_vec = input_names.iter().copied().map(SmallString::from).collect();
        let (added_pools, new_names, default_category) = connection_pool
            .transaction(move |conn| {
                let default_category: SmallString = pool_category::table
                    .select(pool_category::name)
                    .filter(PoolCategory::is_default())
                    .first(conn)?;
                let (pool_ids, new_names) = update::pool::fetch_pools(conn, &ctx, input_names_vec)?;
                let pools = PoolInfo::new_batch_from_ids(conn, &ctx, &pool_ids, FIELDS.into())?;
                Ok::<_, ApiError>((pools, new_names, default_category))
            })
            .await?;

        let mut added_elements = Vec::with_capacity(added_pools.len());
        for pool in added_pools {
            added_elements.push(Element {
                primary_name: pool.primary_name().map(SmallString::from)?,
                category: pool.category().cloned()?,
                post_count: pool.post_count()?,
                class: ElementClass::Added,
            });
        }

        let added_element_names: HashSet<_> = added_elements
            .iter()
            .map(Element::primary_name)
            .chain(new_names.iter().map(|name| name.deref()))
            .collect();
        for element in self.values_mut() {
            if added_element_names.contains(element.primary_name()) {
                element.class = ElementClass::Duplicate;
            }
        }

        let existing_pool_names: HashSet<_> = self.values().map(Element::primary_name).collect();
        let new_elements: Vec<_> = added_elements
            .into_iter()
            .chain(new_names.into_iter().map(|name| Element {
                primary_name: name,
                category: default_category.clone(),
                post_count: 0,
                class: ElementClass::New,
            }))
            .filter(|pool| !existing_pool_names.contains(pool.primary_name()))
            .collect();

        let lowest_current_index = self.first_key_value().map_or(0, |(lowest_index, _)| *lowest_index);
        self.extend((1..).map(|offset| lowest_current_index - offset).zip(new_elements));
        Ok(())
    }
}

impl Deref for ElementMap {
    type Target = BTreeMap<i64, Element>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for ElementMap {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<Vec<MicroPool>> for ElementMap {
    fn from(value: Vec<MicroPool>) -> Self {
        Self((0..).zip(value.into_iter().map(Element::from)).collect())
    }
}

impl<'a> IntoIterator for &'a ElementMap {
    type Item = (&'a i64, &'a Element);
    type IntoIter = std::collections::btree_map::Iter<'a, i64, Element>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

pub type EditPathForm = PathForm<SmallString, EditForm>;

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct EditForm {
    pub version: DateTime,
    pub names: Option<FormField<String>>,
    pub category: Option<FormField<SmallString>>,
    pub description: Option<FormField<LargeString>>,
    pub post_ids: Option<FormField<String>>,
}

impl EditPathForm {
    pub fn initialize(pool: PoolInfo) -> Result<Self, NotRequested> {
        let path = pool.primary_name().map(SmallString::from)?;
        let post_ids = pool.posts().as_deref().map(JoinExt::joined).map(FormField::from).ok();
        let form = EditForm {
            version: pool.version()?,
            names: pool.names.as_ref().map(JoinExt::joined).map(FormField::from),
            category: pool.category.map(FormField::from),
            description: pool.description.map(FormField::from),
            post_ids,
        };
        Ok(Self { path, form })
    }

    pub fn primary_name(&self) -> Result<&str, Infallible> {
        Ok(&self.path)
    }

    pub fn url(&self) -> Result<String, Infallible> {
        Ok(web::pool_url(&self.path))
    }

    pub fn to_body(&self) -> ApiResult<PoolUpdateBody> {
        Ok(PoolUpdateBody {
            version: self.version,
            category: self.category.as_ref().and_then(FormField::form_value_cloned),
            description: self.description.as_ref().and_then(FormField::form_value_cloned),
            names: self
                .names
                .as_ref()
                .and_then(FormField::form_value_deref)
                .map(form::split_into_names),
            posts: self
                .post_ids
                .as_ref()
                .and_then(FormField::form_value_deref)
                .map(form::split_into_ids)
                .transpose()?,
        })
    }
}

pub type MergePathForm = PathForm<SmallString, MergeForm>;

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct MergeForm {
    pub version: DateTime,
    pub target_pool: SmallString,
}

impl MergePathForm {
    pub fn initialize(info: &PoolInfo) -> Result<Self, NotRequested> {
        let path = info.primary_name().map(SmallString::from)?;
        let form = MergeForm {
            version: info.version()?,
            target_pool: SmallString::default(),
        };
        Ok(Self { path, form })
    }

    pub fn primary_name(&self) -> Result<&str, Infallible> {
        Ok(&self.path)
    }

    pub fn url(&self) -> Result<String, Infallible> {
        Ok(web::pool_url(&self.path))
    }
}

pub type DeletePathForm = PathForm<SmallString, DeleteForm>;

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct DeleteForm {
    pub version: DateTime,
    post_count: i64,
}

impl DeletePathForm {
    pub fn initialize(info: &PoolInfo) -> Result<Self, NotRequested> {
        let path = info.primary_name().map(SmallString::from)?;
        let form = DeleteForm {
            version: info.version()?,
            post_count: info.post_count()?,
        };
        Ok(Self { path, form })
    }

    pub fn primary_name(&self) -> Result<&str, Infallible> {
        Ok(&self.path)
    }

    pub fn url(&self) -> Result<String, Infallible> {
        Ok(web::pool_url(&self.path))
    }

    pub fn search_url(&self) -> Result<String, Infallible> {
        Ok(web::pool_search_url(&self.path))
    }

    pub fn post_count(&self) -> Result<i64, Infallible> {
        Ok(self.post_count)
    }

    pub fn to_body(&self) -> DeleteBody {
        DeleteBody { version: self.version }
    }
}
