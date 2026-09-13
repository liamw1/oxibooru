use crate::api::error::ApiResult;
use crate::api::post::PostUpdateBody;
use crate::extract::Ctx;
use crate::model::enums::{MimeType, PostFlag, PostFlags, PostSafety, PostType};
use crate::resource::post::{Mode, PostInfo};
use crate::resource::{JoinExt, NotRequested};
use crate::string::{LargeString, SmallString};
use crate::time::DateTime;
use crate::web::form::pool::ElementMap as PoolElementMap;
use crate::web::form::tag::ElementMap as TagElementMap;
use crate::web::form::{self, FormField};
use crate::web::{self, Message, PathForm, WebResult};
use serde::{Deserialize, Deserializer, Serialize};
use std::convert::Infallible;
use std::str::FromStr;

#[derive(PartialEq, Eq)]
pub enum Focus {
    Tag,
    Pool,
    None,
}

#[derive(Debug, Clone, Copy)]
pub enum Operation {
    Auto,
    Save,
    AddTag,
    AddPool,
    RemoveTag(i64),
    RemovePool(i64),
    Init, // For initializing form: cannot be deserialized into
}

impl FromStr for Operation {
    type Err = &'static str;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "auto" => Some(Self::Auto),
            "save" => Some(Self::Save),
            "add-tag" => Some(Self::AddTag),
            "add-pool" => Some(Self::AddPool),
            _ => {
                if let Some(index) = s.strip_prefix("remove-tag-") {
                    index.parse().map(Self::RemoveTag).ok()
                } else if let Some(index) = s.strip_prefix("remove-pool-") {
                    index.parse().map(Self::RemovePool).ok()
                } else {
                    None
                }
            }
        }
        .ok_or("Failed to parse post operation")
    }
}

impl<'de> Deserialize<'de> for Operation {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = <&str>::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

pub type EditPathForm = PathForm<i64, EditForm>;

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct EditForm {
    pub operation: Operation,
    pub version: DateTime,
    #[serde(default)]
    pub original_flags: Vec<PostFlag>,
    pub safety: Option<FormField<PostSafety>>,
    pub relations: Option<FormField<String>>,
    pub flags: Option<FormField<Vec<PostFlag>>>,
    pub source: Option<FormField<LargeString>>,
    pub description: Option<FormField<LargeString>>,
    pub tags: Option<FormField<TagElementMap>>,
    pub pools: Option<FormField<PoolElementMap>>,
    #[serde(rename = "type")]
    type_: PostType,
    mime_type: MimeType,
    canvas_width: i32,
    canvas_height: i32,
    content_url: String,
    thumbnail_url: String,
    new_tags: Option<SmallString>,
    new_pools: Option<SmallString>,
}

impl EditPathForm {
    pub fn initialize(post: PostInfo) -> Result<Self, NotRequested> {
        let path = post.id()?;
        let form = EditForm {
            operation: Operation::Init,
            version: post.version()?,
            original_flags: post.flags().map(PostFlags::to_vec)?,
            type_: post.type_()?,
            mime_type: post.mime_type()?,
            canvas_width: post.canvas_width()?,
            canvas_height: post.canvas_height()?,
            content_url: post.content_url().cloned()?,
            thumbnail_url: post.thumbnail_url().cloned()?,
            safety: post.safety.map(FormField::from),
            relations: post.relations.as_ref().map(JoinExt::joined).map(FormField::from),
            flags: post.flags.map(PostFlags::to_vec).map(FormField::from),
            source: post.source.map(FormField::from),
            description: post.description.map(FormField::from),
            tags: post.tags.map(TagElementMap::from).map(FormField::from),
            pools: post.pools.map(PoolElementMap::from).map(FormField::from),
            new_tags: None,
            new_pools: None,
        };
        Ok(Self { path, form })
    }

    pub fn id(&self) -> Result<i64, Infallible> {
        Ok(self.path)
    }

    pub fn type_(&self) -> Result<PostType, Infallible> {
        Ok(self.type_)
    }

    pub fn mime_type(&self) -> Result<MimeType, Infallible> {
        Ok(self.mime_type)
    }

    pub fn canvas_width(&self) -> Result<i32, Infallible> {
        Ok(self.canvas_width)
    }

    pub fn canvas_height(&self) -> Result<i32, Infallible> {
        Ok(self.canvas_height)
    }

    pub fn content_url(&self) -> Result<&str, Infallible> {
        Ok(&self.content_url)
    }

    pub fn thumbnail_url(&self) -> Result<&str, Infallible> {
        Ok(&self.thumbnail_url)
    }

    pub fn flags(&self) -> Result<PostFlags, Infallible> {
        Ok(PostFlags::from_slice(&self.original_flags))
    }

    pub fn tag_count(&self) -> usize {
        self.tags.as_ref().map(|tags| tags.current().len()).unwrap_or(0)
    }

    pub fn pool_count(&self) -> usize {
        self.pools.as_ref().map(|pools| pools.current().len()).unwrap_or(0)
    }

    pub fn note_count(&self) -> usize {
        0 // TODO
    }

    pub fn url<T: Serialize>(&self, mode: Mode, params: &T) -> Result<String, serde_urlencoded::ser::Error> {
        match mode {
            Mode::View => web::post_url(self.path, params),
            Mode::Edit => web::post_edit_url(self.path, params),
        }
    }

    pub fn to_body(&self) -> ApiResult<PostUpdateBody> {
        Ok(PostUpdateBody {
            version: self.version,
            safety: self.safety.as_ref().and_then(FormField::form_value).copied(),
            source: self.source.as_ref().and_then(FormField::form_value_cloned),
            description: self.description.as_ref().and_then(FormField::form_value_cloned),
            relations: self
                .relations
                .as_ref()
                .and_then(FormField::form_value_deref)
                .map(form::split_into_ids)
                .transpose()?,
            tags: self
                .tags
                .as_ref()
                .and_then(FormField::form_value)
                .map(TagElementMap::names),
            pools: self
                .pools
                .as_ref()
                .and_then(FormField::form_value)
                .map(PoolElementMap::names),
            notes: None,
            flags: self.flags.as_ref().and_then(FormField::form_value_cloned),
            content_token: None,
            content_url: None,
            thumbnail_token: None,
            thumbnail_url: None,
        })
    }

    pub fn with_tag_removed(mut self, index: i64) -> (Self, Focus, Message) {
        if let Some(tags) = &mut self.tags {
            tags.current.remove(&index);
        }
        (self, Focus::None, Message::None)
    }

    pub fn with_pool_removed(mut self, index: i64) -> (Self, Focus, Message) {
        if let Some(pools) = &mut self.pools {
            pools.current.remove(&index);
        }
        (self, Focus::None, Message::None)
    }

    pub async fn with_new_tags(mut self, ctx: Ctx) -> WebResult<(Self, Focus, Message)> {
        if let Some(new_names) = self.new_tags.take()
            && !new_names.is_empty()
        {
            self.tags
                .get_or_insert_default()
                .current
                .append_tags(ctx, &new_names)
                .await?;
        }
        Ok((self, Focus::None, Message::None))
    }

    pub async fn with_new_pools(mut self, ctx: Ctx) -> WebResult<(Self, Focus, Message)> {
        if let Some(new_names) = self.new_pools.take()
            && !new_names.is_empty()
        {
            self.pools
                .get_or_insert_default()
                .current
                .append_pools(ctx, &new_names)
                .await?;
        }
        Ok((self, Focus::None, Message::None))
    }

    pub async fn auto_modify(self, ctx: Ctx) -> WebResult<(Self, Focus, Message)> {
        let has_tag_input = !self.new_tags.as_deref().is_none_or(str::is_empty);
        let has_pool_input = !self.new_pools.as_deref().is_none_or(str::is_empty);
        let focus = match (has_tag_input, has_pool_input) {
            (false | true, true) => Focus::Pool,
            (true, false) => Focus::Tag,
            (false, false) => Focus::None,
        };

        let (form, ..) = self.with_new_tags(ctx.clone()).await?;
        let (form, ..) = form.with_new_pools(ctx).await?;
        Ok((form, focus, Message::None))
    }
}
