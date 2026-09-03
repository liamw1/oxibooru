use crate::api::tag::TagUpdateBody;
use crate::extract::{Ctx, DeleteBody};
use crate::resource::NotRequested;
use crate::resource::tag::TagInfo;
use crate::string::{self, LargeString, SmallString};
use crate::time::DateTime;
use crate::web::form::{FormField, TagMap};
use crate::web::{Message, PathForm, WebResult};
use serde::{Deserialize, Deserializer};
use std::convert::Infallible;
use std::str::FromStr;

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
    pub names: Option<FormField<String>>,
    pub category: Option<FormField<SmallString>>,
    pub implications: Option<FormField<TagMap>>,
    pub suggestions: Option<FormField<TagMap>>,
    pub description: Option<FormField<LargeString>>,
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
            names,
            category: info.category.map(FormField::from),
            implications: implications.map(FormField::from),
            suggestions: suggestions.map(FormField::from),
            description: info.description.map(FormField::from),
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
