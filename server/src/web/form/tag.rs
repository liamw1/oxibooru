use crate::api::error::ApiError;
use crate::api::tag::TagUpdateBody;
use crate::extract::{Ctx, DeleteBody};
use crate::model::tag_category::TagCategory;
use crate::resource::tag::{Field, MicroTag, TagInfo};
use crate::resource::{JoinExt, NotRequested};
use crate::schema::tag_category;
use crate::string::{LargeString, SmallString};
use crate::time::DateTime;
use crate::update::tag::FetchMode;
use crate::web::form::{self, FormField};
use crate::web::{Message, PathForm, WebResult};
use crate::{string, update, web};
use diesel::{QueryDsl, RunQueryDsl};
use serde::{Deserialize, Deserializer};
use std::collections::{BTreeMap, HashSet};
use std::convert::Infallible;
use std::ops::{Deref, DerefMut};
use std::str::FromStr;
use std::sync::Arc;
use strum::Display;

#[derive(PartialEq, Eq)]
pub enum Focus {
    Implication,
    Suggestion,
    None,
}

#[derive(Debug, Clone, Copy)]
pub enum Operation {
    Auto,
    Save,
    AddImplication,
    AddSuggestion,
    RemoveImplication(i64),
    RemoveSuggestion(i64),
    Init, // For initializing form: cannot be deserialized into
}

impl FromStr for Operation {
    type Err = &'static str;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "auto" => Some(Self::Auto),
            "save" => Some(Self::Save),
            "add-implication" => Some(Self::AddImplication),
            "add-suggestion" => Some(Self::AddSuggestion),
            _ => {
                if let Some(index) = s.strip_prefix("remove-implication-") {
                    index.parse().map(Self::RemoveImplication).ok()
                } else if let Some(index) = s.strip_prefix("remove-suggestion-") {
                    index.parse().map(Self::RemoveSuggestion).ok()
                } else {
                    None
                }
            }
        }
        .ok_or("Failed to parse tag operation")
    }
}

impl<'de> Deserialize<'de> for Operation {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = <&str>::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Copy, Default, Display)]
#[strum(serialize_all = "lowercase")]
pub enum ElementClass {
    New,
    Added,
    Duplicate,
    Implication,
    #[default]
    #[strum(serialize = "")]
    None,
}

#[derive(Deserialize)]
pub struct Element {
    primary_name: SmallString,
    pub category: SmallString,
    pub usages: i64,
    #[serde(skip)]
    pub class: ElementClass,
}

impl Element {
    pub fn from_microtag(tag: MicroTag, class: ElementClass) -> Self {
        Self {
            primary_name: tag.names[0].clone(),
            category: tag.category,
            usages: tag.usages,
            class,
        }
    }

    pub fn primary_name(&self) -> &str {
        &self.primary_name
    }

    pub fn url(&self) -> String {
        web::tag_url(&self.primary_name)
    }

    pub fn search_url(&self) -> String {
        web::tag_search_url(&self.primary_name)
    }

    pub fn class(&self) -> ElementClass {
        self.class
    }
}

impl From<MicroTag> for Element {
    fn from(tag: MicroTag) -> Self {
        Self::from_microtag(tag, ElementClass::None)
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

    async fn append_tags(&mut self, Ctx(ctx, connection_pool): &Ctx, joined_names: &str) -> WebResult<()> {
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
                    let (tag_ids, new_names) = update::tag::fetch_tags(conn, &ctx, tag_names, FetchMode::Deep)?;
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
                element.class = ElementClass::Duplicate;
            }
        }

        let existing_tags: HashSet<_> = self.values().map(Element::primary_name).collect();
        let new_elements: Vec<_> = micro_tags
            .into_iter()
            .map(|tag| {
                let class = if added_names.contains(tag.primary_name()) {
                    ElementClass::Added
                } else {
                    ElementClass::Implication
                };
                Element::from_microtag(tag, class)
            })
            .chain(new_names.into_iter().map(|name| {
                let tag = MicroTag {
                    names: Arc::from([name]),
                    category: default_category.clone(),
                    usages: 0,
                };
                Element::from_microtag(tag, ElementClass::New)
            }))
            .filter(|tag| !existing_tags.contains(tag.primary_name()))
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

impl From<Vec<MicroTag>> for ElementMap {
    fn from(value: Vec<MicroTag>) -> Self {
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
    pub operation: Operation,
    pub version: DateTime,
    pub names: Option<FormField<String>>,
    pub category: Option<FormField<SmallString>>,
    pub implications: Option<FormField<ElementMap>>,
    pub suggestions: Option<FormField<ElementMap>>,
    pub description: Option<FormField<LargeString>>,
    new_implications: Option<String>,
    new_suggestions: Option<String>,
}

impl EditPathForm {
    pub fn initialize(info: TagInfo) -> Result<Self, NotRequested> {
        let path = info.primary_name().map(SmallString::from)?;
        let form = EditForm {
            operation: Operation::Init,
            version: info.version()?,
            names: info.names.as_ref().map(JoinExt::joined).map(FormField::from),
            category: info.category.map(FormField::from),
            implications: info.implications.map(ElementMap::from).map(FormField::from),
            suggestions: info.suggestions.map(ElementMap::from).map(FormField::from),
            description: info.description.map(FormField::from),
            new_implications: None,
            new_suggestions: None,
        };
        Ok(Self { path, form })
    }

    pub fn primary_name(&self) -> Result<&str, Infallible> {
        Ok(&self.path)
    }

    pub fn url(&self) -> Result<String, Infallible> {
        Ok(web::tag_url(&self.path))
    }

    pub fn to_body(&self) -> TagUpdateBody {
        TagUpdateBody {
            version: self.version,
            category: self.category.as_ref().and_then(FormField::form_value_cloned),
            description: self.description.as_ref().and_then(FormField::form_value_cloned),
            names: self
                .names
                .as_ref()
                .and_then(FormField::form_value_deref)
                .map(form::split_into_names),
            implications: self
                .implications
                .as_ref()
                .and_then(FormField::form_value)
                .map(ElementMap::names),
            suggestions: self
                .suggestions
                .as_ref()
                .and_then(FormField::form_value)
                .map(ElementMap::names),
        }
    }

    pub fn with_implication_removed(mut self, index: i64) -> (Self, Focus, Message) {
        if let Some(implications) = &mut self.implications {
            implications.current.remove(&index);
        }
        (self, Focus::None, Message::None)
    }

    pub fn with_suggestion_removed(mut self, index: i64) -> (Self, Focus, Message) {
        if let Some(suggestions) = &mut self.suggestions {
            suggestions.current.remove(&index);
        }
        (self, Focus::None, Message::None)
    }

    pub async fn with_new_implications(mut self, ctx: Ctx) -> WebResult<(Self, Focus, Message)> {
        if let Some(new_names) = self.new_implications.take()
            && !new_names.is_empty()
        {
            self.implications
                .get_or_insert_default()
                .current
                .append_tags(&ctx, &new_names)
                .await?;
        }
        Ok((self, Focus::None, Message::None))
    }

    pub async fn with_new_suggestions(mut self, ctx: Ctx) -> WebResult<(Self, Focus, Message)> {
        if let Some(new_names) = self.new_suggestions.take()
            && !new_names.is_empty()
        {
            self.suggestions
                .get_or_insert_default()
                .current
                .append_tags(&ctx, &new_names)
                .await?;
        }
        Ok((self, Focus::None, Message::None))
    }

    pub async fn auto_modify(self, ctx: Ctx) -> WebResult<(Self, Focus, Message)> {
        let has_implication_input = !self.new_implications.as_deref().is_none_or(str::is_empty);
        let has_suggestion_input = !self.new_suggestions.as_deref().is_none_or(str::is_empty);
        let focus = match (has_implication_input, has_suggestion_input) {
            (false | true, true) => Focus::Suggestion,
            (true, false) => Focus::Implication,
            (false, false) => Focus::None,
        };

        let (form, ..) = self.with_new_implications(ctx.clone()).await?;
        let (form, ..) = form.with_new_suggestions(ctx).await?;
        Ok((form, focus, Message::None))
    }
}

pub type MergePathForm = PathForm<SmallString, MergeForm>;

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct MergeForm {
    pub version: DateTime,
    pub target_tag: SmallString,
}

impl MergePathForm {
    pub fn initialize(tag: &TagInfo) -> Result<Self, NotRequested> {
        let path = tag.primary_name().map(SmallString::from)?;
        let form = MergeForm {
            version: tag.version()?,
            target_tag: SmallString::default(),
        };
        Ok(Self { path, form })
    }

    pub fn primary_name(&self) -> Result<&str, Infallible> {
        Ok(&self.path)
    }

    pub fn url(&self) -> Result<String, Infallible> {
        Ok(web::tag_url(&self.path))
    }
}

pub type DeletePathForm = PathForm<SmallString, DeleteForm>;

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct DeleteForm {
    pub version: DateTime,
    usages: i64,
}

impl DeletePathForm {
    pub fn initialize(info: &TagInfo) -> Result<Self, NotRequested> {
        let path = info.primary_name().map(SmallString::from)?;
        let form = DeleteForm {
            version: info.version()?,
            usages: info.usages()?,
        };
        Ok(Self { path, form })
    }

    pub fn primary_name(&self) -> Result<&str, Infallible> {
        Ok(&self.path)
    }

    pub fn url(&self) -> Result<String, Infallible> {
        Ok(web::tag_url(&self.path))
    }

    pub fn search_url(&self) -> Result<String, Infallible> {
        Ok(web::tag_search_url(&self.path))
    }

    pub fn usages(&self) -> Result<i64, Infallible> {
        Ok(self.usages)
    }

    pub fn to_body(&self) -> DeleteBody {
        DeleteBody { version: self.version }
    }
}
