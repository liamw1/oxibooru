use crate::api::error::ApiError;
use crate::extract::Ctx;
use crate::model::tag_category::TagCategory;
use crate::resource::tag::{Field, MicroTag, TagInfo};
use crate::schema::tag_category;
use crate::string::SmallString;
use crate::update::tag::FetchMode;
use crate::web::WebResult;
use crate::{string, update};
use diesel::{QueryDsl, RunQueryDsl};
use serde::{Deserialize, Deserializer};
use std::collections::{BTreeMap, HashSet};
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use strum::Display;

pub mod tag;

#[derive(Deserialize)]
pub struct FormField<T> {
    #[serde(default)]
    current: T,
    #[serde(default = "some_default")]
    original: Option<T>,
}

impl<T> FormField<T> {
    pub fn current(&self) -> &T {
        &self.current
    }

    pub fn original(&self) -> &T {
        self.original.as_ref().unwrap_or(&self.current)
    }
}

impl<T: Eq> FormField<T> {
    pub fn form_value(&self) -> Option<&T> {
        self.original
            .as_ref()
            .is_none_or(|original| self.current != *original)
            .then_some(&self.current)
    }

    pub fn form_value_cloned(&self) -> Option<T>
    where
        T: Clone,
    {
        self.original
            .as_ref()
            .is_none_or(|original| self.current != *original)
            .then_some(self.current.clone())
    }

    pub fn form_value_deref<R>(&self) -> Option<&R>
    where
        T: Deref<Target = R>,
        R: ?Sized,
    {
        self.original
            .as_ref()
            .is_none_or(|original| self.current != *original)
            .then_some(&*self.current)
    }
}

impl<T: Default> Default for FormField<T> {
    fn default() -> Self {
        Self {
            current: T::default(),
            original: Some(T::default()),
        }
    }
}

impl<T> From<T> for FormField<T> {
    fn from(value: T) -> Self {
        Self {
            current: value,
            original: None,
        }
    }
}

#[derive(Clone, Copy, Display)]
pub enum TagElementClass {
    New,
    Added,
    Duplicate,
    Implication,
    #[strum(serialize = "")]
    None,
}

#[derive(Deserialize)]
#[serde(from = "MicroTag")]
pub struct TagElement {
    tag: MicroTag,
    class: TagElementClass,
}

impl TagElement {
    pub fn class(&self) -> TagElementClass {
        self.class
    }
}

impl Deref for TagElement {
    type Target = MicroTag;
    fn deref(&self) -> &Self::Target {
        &self.tag
    }
}

impl From<MicroTag> for TagElement {
    fn from(tag: MicroTag) -> Self {
        Self {
            tag,
            class: TagElementClass::None,
        }
    }
}

impl PartialEq for TagElement {
    fn eq(&self, other: &Self) -> bool {
        self.primary_name() == other.primary_name()
    }
}

impl Eq for TagElement {}

#[derive(Default, PartialEq, Eq, Deserialize)]
pub struct TagMap(BTreeMap<i64, TagElement>);

impl TagMap {
    pub fn names(&self) -> Vec<SmallString> {
        self.0
            .values()
            .map(|tag| SmallString::from(tag.primary_name()))
            .collect()
    }

    async fn append_tags(
        &mut self,
        Ctx(ctx, connection_pool): &Ctx,
        joined_names: &str,
        fetch_mode: FetchMode,
    ) -> WebResult<()> {
        const FIELDS: [Field; 3] = [Field::Category, Field::Names, Field::Usages];

        let added_names: HashSet<_> = string::split_unescaped_whitespace(joined_names).collect();
        let tag_names = added_names.iter().copied().map(SmallString::from).collect();
        let (tags, new_names, default_category) = connection_pool
            .transaction({
                let ctx = ctx.clone();
                move |conn| {
                    let default_category: SmallString = tag_category::table
                        .select(tag_category::name)
                        .filter(TagCategory::is_default())
                        .first(conn)?;
                    let (tag_ids, new_names) = update::tag::fetch_tags(conn, &ctx, tag_names, fetch_mode)?;
                    let tags = TagInfo::new_batch_from_ids(conn, &tag_ids, FIELDS.into())?;
                    Ok::<_, ApiError>((tags, new_names, default_category))
                }
            })
            .await?;

        let mut micro_tags = Vec::with_capacity(tags.len());
        for tag in tags {
            micro_tags.push(MicroTag {
                names: tag.names().map(Vec::as_slice).map(Arc::from)?,
                category: tag.category().cloned()?,
                usages: tag.usages()?,
            });
        }

        let tag_names: HashSet<_> = micro_tags
            .iter()
            .map(MicroTag::primary_name)
            .chain(new_names.iter().map(|name| name.deref()))
            .collect();
        for element in self.values_mut() {
            if tag_names.contains(element.primary_name()) {
                element.class = TagElementClass::Duplicate;
            }
        }

        let existing_tags: HashSet<_> = self.values().map(|tag| tag.primary_name()).collect();
        let new_elements: Vec<_> = micro_tags
            .into_iter()
            .map(|tag| {
                let class = if added_names.contains(tag.primary_name()) {
                    TagElementClass::Added
                } else {
                    TagElementClass::Implication
                };
                TagElement { tag, class }
            })
            .chain(new_names.into_iter().map(|name| {
                let tag = MicroTag {
                    names: Arc::from([name]),
                    category: default_category.clone(),
                    usages: 0,
                };
                TagElement {
                    tag,
                    class: TagElementClass::New,
                }
            }))
            .filter(|tag| !existing_tags.contains(tag.primary_name()))
            .collect();

        let lowest_current_index = self.first_key_value().map_or(0, |(lowest_index, _)| *lowest_index);
        self.extend((1..).map(|offset| lowest_current_index - offset).zip(new_elements));
        Ok(())
    }
}

impl Deref for TagMap {
    type Target = BTreeMap<i64, TagElement>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for TagMap {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<Vec<MicroTag>> for TagMap {
    fn from(value: Vec<MicroTag>) -> Self {
        Self((0..).zip(value.into_iter().map(TagElement::from)).collect())
    }
}

impl<'a> IntoIterator for &'a TagMap {
    type Item = (&'a i64, &'a TagElement);
    type IntoIter = std::collections::btree_map::Iter<'a, i64, TagElement>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

fn some_default<T: Default>() -> Option<T> {
    Some(T::default())
}

fn checkbox<'de, D: Deserializer<'de>>(deserializer: D) -> Result<bool, D::Error> {
    Option::<&str>::deserialize(deserializer).map(|v| v.is_some())
}
