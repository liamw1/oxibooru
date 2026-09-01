use crate::api::error::ApiError;
use crate::api::tag::TagUpdateBody;
use crate::extract::{Ctx, DeleteBody, PathForm};
use crate::resource::NotRequested;
use crate::resource::tag::{Field, MicroTag, TagInfo};
use crate::string::{LargeString, SmallString};
use crate::time::DateTime;
use crate::update::tag::FetchMode;
use crate::web::form::{FormField, TagMap};
use crate::web::{Message, WebResult};
use crate::{string, update};
use serde::{Deserialize, Deserializer};
use std::collections::{BTreeMap, HashSet};
use std::convert::Infallible;
use std::str::FromStr;
use std::sync::Arc;

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

pub type EditPathForm = PathForm<SmallString, EditForm>;

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct EditForm {
    pub operation: Operation,
    pub category: Option<FormField<SmallString>>,
    pub description: Option<FormField<LargeString>>,
    pub names: Option<FormField<String>>,
    pub implications: Option<FormField<TagMap>>,
    pub suggestions: Option<FormField<TagMap>>,
    version: DateTime,
    new_implications: Option<SmallString>,
    new_suggestions: Option<SmallString>,
}

impl EditPathForm {
    pub fn initialize(info: TagInfo) -> Result<Self, NotRequested> {
        let path = info.primary_name().map(SmallString::from)?;
        let version = info.version()?;
        let names = info.joined_names().ok().map(FormField::from);
        let implications = info.implications.map(TagMap::from);
        let suggestions = info.suggestions.map(TagMap::from);
        let form = EditForm {
            operation: Operation::Init,
            category: info.category.map(FormField::from),
            description: info.description.map(FormField::from),
            names,
            implications: implications.map(FormField::from),
            suggestions: suggestions.map(FormField::from),
            version,
            new_implications: None,
            new_suggestions: None,
        };
        Ok(Self { path, form })
    }

    pub fn version(&self) -> Result<DateTime, Infallible> {
        Ok(self.version)
    }

    pub fn primary_name(&self) -> Result<&str, Infallible> {
        Ok(&self.path)
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

    pub(in crate::web) fn with_implication_removed(mut self, index: i64) -> (Self, Focus, Message) {
        if let Some(implications) = &mut self.implications {
            implications.current.remove(&index);
        }
        (self, Focus::None, Message::None)
    }

    pub(in crate::web) fn with_suggestion_removed(mut self, index: i64) -> (Self, Focus, Message) {
        if let Some(suggestions) = &mut self.suggestions {
            suggestions.current.remove(&index);
        }
        (self, Focus::None, Message::None)
    }

    pub(in crate::web) async fn with_new_implications(mut self, ctx: Ctx) -> WebResult<(Self, Focus, Message)> {
        if let Some(new_names) = self.new_implications.take()
            && !new_names.is_empty()
        {
            let implications = self.implications.get_or_insert_default();
            let new_tags = get_tag_info(&ctx, &new_names, implications.original()).await?;
            implications.current.append_tags(new_tags);
        }
        Ok((self, Focus::None, Message::None))
    }

    pub(in crate::web) async fn with_new_suggestions(mut self, ctx: Ctx) -> WebResult<(Self, Focus, Message)> {
        if let Some(new_names) = self.new_suggestions.take()
            && !new_names.is_empty()
        {
            let suggestions = self.suggestions.get_or_insert_default();
            let new_tags = get_tag_info(&ctx, &new_names, suggestions.original()).await?;
            suggestions.current.append_tags(new_tags);
        }
        Ok((self, Focus::None, Message::None))
    }

    pub(in crate::web) async fn auto_modify(self, ctx: Ctx) -> WebResult<(Self, Focus, Message)> {
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
    pub fn initialize(info: &TagInfo) -> Result<Self, NotRequested> {
        let path = info.primary_name().map(SmallString::from)?;
        let form = MergeForm {
            version: info.version()?,
            target_tag: SmallString::default(),
        };
        Ok(Self { path, form })
    }

    pub fn version(&self) -> Result<DateTime, Infallible> {
        Ok(self.version)
    }

    pub fn primary_name(&self) -> Result<&str, Infallible> {
        Ok(&self.path)
    }
}

pub type DeletePathForm = PathForm<SmallString, DeleteForm>;

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct DeleteForm {
    version: DateTime,
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

    pub fn version(&self) -> Result<DateTime, Infallible> {
        Ok(self.version)
    }

    pub fn primary_name(&self) -> Result<&str, Infallible> {
        Ok(&self.path)
    }

    pub fn usages(&self) -> Result<i64, Infallible> {
        Ok(self.usages)
    }

    pub fn to_body(&self) -> DeleteBody {
        DeleteBody { version: self.version }
    }
}

async fn get_tag_info(
    Ctx(ctx, connection_pool): &Ctx,
    joined_names: &str,
    existing_tags: &BTreeMap<i64, MicroTag>,
) -> WebResult<Vec<MicroTag>> {
    const FIELDS: [Field; 3] = [Field::Category, Field::Names, Field::Usages];

    let tag_names = string::split_unescaped_whitespace(joined_names)
        .map(SmallString::from)
        .collect();
    let (tag_ids, _) = connection_pool
        .transaction({
            let ctx = ctx.clone();
            move |conn| update::tag::get_or_create_tags(conn, &ctx, tag_names, FetchMode::Deep)
        })
        .await?;
    let tags = connection_pool
        .transaction(move |conn| TagInfo::new_batch_from_ids(conn, &tag_ids, FIELDS.into()).map_err(ApiError::from))
        .await?;

    let mut micro_tags = Vec::new();
    let existing_names: HashSet<_> = existing_tags.values().map(MicroTag::primary_name).collect();
    for tag in tags {
        if existing_names.contains(tag.primary_name()?) {
            continue;
        }

        micro_tags.push(MicroTag {
            names: tag.names().map(Vec::as_slice).map(Arc::from)?,
            category: tag.category().cloned()?,
            usages: tag.usages()?,
        });
    }
    Ok(micro_tags)
}
