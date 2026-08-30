use crate::api::error::ApiError;
use crate::api::tag::TagUpdateBody;
use crate::extract::Ctx;
use crate::resource::NotRequested;
use crate::resource::field::Mask;
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
    operation: Operation,
    new_implications: Option<SmallString>,
    new_suggestions: Option<SmallString>,
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
            operation: Operation::Init,
            new_implications: None,
            new_suggestions: None,
        })
    }

    pub fn version(&self) -> Result<DateTime, Infallible> {
        Ok(self.version)
    }

    pub fn primary_name(&self) -> Result<&str, Infallible> {
        Ok(&self.primary_name)
    }

    pub fn operation(&self) -> Operation {
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
            let implications = self.implications.get_or_insert_default();
            let new_tags = get_tag_info(&ctx, &new_names, implications.original()).await?;
            implications.current.append_tags(new_tags);
        }
        Ok((self, Focus::None, Message::None))
    }

    pub async fn with_new_suggestions(mut self, ctx: Ctx) -> WebResult<(Self, Focus, Message)> {
        if let Some(new_names) = self.new_suggestions.take()
            && !new_names.is_empty()
        {
            let suggestions = self.suggestions.get_or_insert_default();
            let new_tags = get_tag_info(&ctx, &new_names, suggestions.original()).await?;
            suggestions.current.append_tags(new_tags);
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

async fn get_tag_info(
    Ctx(ctx, connection_pool): &Ctx,
    joined_names: &str,
    existing_tags: &BTreeMap<i64, MicroTag>,
) -> WebResult<Vec<MicroTag>> {
    let fields: Mask<_> = [Field::Category, Field::Names, Field::Usages].into();

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
        .transaction(move |conn| TagInfo::new_batch_from_ids(conn, &tag_ids, fields).map_err(ApiError::from))
        .await?;

    let mut micro_tags = Vec::new();
    let existing_names: HashSet<_> = existing_tags.iter().map(|(_, tag)| tag.primary_name()).collect();
    for tag in tags {
        if existing_names.contains(tag.primary_name()?) {
            continue;
        }

        micro_tags.push(MicroTag {
            names: tag.names().map(Vec::as_slice).map(Arc::from)?,
            category: tag.category().cloned()?,
            usages: tag.usages()?,
        })
    }
    Ok(micro_tags)
}
