use crate::api::error::ApiResult;
use crate::api::pool::PoolUpdateBody;
use crate::extract::DeleteBody;
use crate::resource::NotRequested;
use crate::resource::pool::PoolInfo;
use crate::string::{self, LargeString, SmallString};
use crate::time::DateTime;
use crate::web::PathForm;
use crate::web::form::FormField;
use serde::Deserialize;
use std::convert::Infallible;

pub type EditPathForm = PathForm<SmallString, EditForm>;

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct EditForm {
    pub names: Option<FormField<String>>,
    pub category: Option<FormField<SmallString>>,
    pub description: Option<FormField<LargeString>>,
    pub post_ids: Option<FormField<String>>,
    version: DateTime,
}

impl EditPathForm {
    pub fn initialize(info: PoolInfo) -> Result<Self, NotRequested> {
        let path = info.primary_name().map(SmallString::from)?;
        let version = info.version()?;
        let names = info.joined_names().ok().map(FormField::from);
        let post_ids = info.joined_post_ids().ok().map(FormField::from);
        let form = EditForm {
            names,
            category: info.category.map(FormField::from),
            description: info.description.map(FormField::from),
            post_ids,
            version,
        };
        Ok(Self { path, form })
    }

    pub fn version(&self) -> Result<DateTime, Infallible> {
        Ok(self.version)
    }

    pub fn primary_name(&self) -> Result<&str, Infallible> {
        Ok(&self.path)
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
                .map(string::split_into_list),
            posts: self
                .post_ids
                .as_ref()
                .and_then(FormField::form_value_deref)
                .map(|joined_ids| {
                    string::split_unescaped_whitespace(joined_ids)
                        .map(|id| id.parse())
                        .collect::<Result<_, _>>()
                })
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

    pub fn version(&self) -> Result<DateTime, Infallible> {
        Ok(self.version)
    }

    pub fn primary_name(&self) -> Result<&str, Infallible> {
        Ok(&self.path)
    }

    pub fn post_count(&self) -> Result<i64, Infallible> {
        Ok(self.post_count)
    }

    pub fn to_body(&self) -> DeleteBody {
        DeleteBody { version: self.version }
    }
}
