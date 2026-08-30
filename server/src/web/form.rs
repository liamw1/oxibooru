use crate::api::tag::TagUpdateBody;
use crate::extract::{Ctx, Json, Path, Query};
use crate::resource::NotRequested;
use crate::resource::field::Mask;
use crate::resource::tag::{Field, MicroTag, TagInfo};
use crate::string::{LargeString, SmallString};
use crate::time::DateTime;
use crate::web::WebResult;
use crate::{api, string};
use serde::{Deserialize, Deserializer};
use std::collections::BTreeMap;
use std::convert::Infallible;
use std::ops::{Deref, DerefMut};
use std::str::FromStr;
use std::sync::Arc;

#[derive(Debug, Clone, Copy)]
pub enum TagOperation {
    Save,
    AddImplication,
    AddSuggestion,
    RemoveImplication(i64),
    RemoveSuggestion(i64),
    Init, // For initializing form: cannot be deserialized into
}

impl FromStr for TagOperation {
    type Err = &'static str;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
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

impl<'de> Deserialize<'de> for TagOperation {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = <&str>::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Deserialize)]
pub struct FormField<T> {
    #[serde(default)]
    current: T,
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

impl<T> From<T> for FormField<T> {
    fn from(value: T) -> Self {
        Self {
            current: value,
            original: None,
        }
    }
}

#[derive(Debug, Default, PartialEq, Eq, Deserialize)]
pub struct TagMap(BTreeMap<i64, MicroTag>);

impl TagMap {
    pub fn names(&self) -> Vec<SmallString> {
        self.0
            .values()
            .map(MicroTag::primary_name)
            .map(SmallString::from)
            .collect()
    }
}

impl Deref for TagMap {
    type Target = BTreeMap<i64, MicroTag>;
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
        Self((0..).zip(value).collect())
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct EditForm {
    pub version: DateTime,
    pub primary_name: SmallString,
    pub category: Option<FormField<SmallString>>,
    pub description: Option<FormField<LargeString>>,
    pub names: Option<FormField<String>>,
    pub implications: Option<FormField<TagMap>>,
    pub suggestions: Option<FormField<TagMap>>,
    operation: TagOperation,
    new_implication: Option<SmallString>,
    new_suggestion: Option<SmallString>,
}

impl EditForm {
    pub fn initialize(info: TagInfo) -> Result<Self, NotRequested> {
        let version = info.version()?;
        let primary_name = info.primary_name().map(SmallString::from)?;
        let names = info.joined_names().ok().map(FormField::from);
        let implications = info.implications.map(TagMap::from);
        let suggestions = info.suggestions.map(TagMap::from);

        Ok(Self {
            version,
            primary_name,
            category: info.category.map(FormField::from),
            description: info.description.map(FormField::from),
            names,
            implications: implications.map(FormField::from),
            suggestions: suggestions.map(FormField::from),
            operation: TagOperation::Init,
            new_implication: None,
            new_suggestion: None,
        })
    }

    pub fn version(&self) -> Result<DateTime, Infallible> {
        Ok(self.version)
    }

    pub fn primary_name(&self) -> Result<&str, Infallible> {
        Ok(&self.primary_name)
    }

    pub fn operation(&self) -> TagOperation {
        self.operation
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
                .map(string::split_into_list),
            implications: self
                .implications
                .as_ref()
                .and_then(FormField::form_value)
                .map(TagMap::names),
            suggestions: self
                .suggestions
                .as_ref()
                .and_then(FormField::form_value)
                .map(TagMap::names),
        }
    }

    pub fn with_implication_removed(mut self, index: i64) -> Self {
        if let Some(implications) = &mut self.implications {
            implications.current.remove(&index);
        }
        self
    }

    pub fn with_suggestion_removed(mut self, index: i64) -> Self {
        if let Some(suggestions) = &mut self.suggestions {
            suggestions.current.remove(&index);
        }
        self
    }

    pub async fn with_new_implication(mut self, ctx: Ctx) -> WebResult<Self> {
        if let Some(tag_name) = self.new_implication.take()
            && let Some(implications) = &mut self.implications
        {
            let fields: Mask<_> = [Field::Category, Field::Names, Field::Usages].into();
            let Json(tag_info) = api::tag::get(ctx, Path(tag_name), Query(fields.into())).await?;
            let micro_tag = MicroTag {
                names: tag_info.names().map(Vec::as_slice).map(Arc::from)?,
                category: tag_info.category().cloned()?,
                usages: tag_info.usages()?,
            };

            let index = implications
                .current
                .first_key_value()
                .map_or(0, |(first_index, _)| first_index - 1);
            implications.current.insert(index, micro_tag);
        }
        Ok(self)
    }

    pub async fn with_new_suggestion(mut self, ctx: Ctx) -> WebResult<Self> {
        if let Some(tag_name) = self.new_suggestion.take()
            && let Some(suggestions) = &mut self.suggestions
        {
            let fields: Mask<_> = [Field::Category, Field::Names, Field::Usages].into();
            let Json(tag_info) = api::tag::get(ctx, Path(tag_name), Query(fields.into())).await?;
            let micro_tag = MicroTag {
                names: tag_info.names().map(Vec::as_slice).map(Arc::from)?,
                category: tag_info.category().cloned()?,
                usages: tag_info.usages()?,
            };

            let index = suggestions
                .current
                .first_key_value()
                .map_or(0, |(first_index, _)| first_index - 1);
            suggestions.current.insert(index, micro_tag);
        }
        Ok(self)
    }
}
