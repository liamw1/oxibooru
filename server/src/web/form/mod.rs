use crate::string::{self, SmallString};
use serde::Deserialize;
use std::num::ParseIntError;
use std::ops::Deref;

pub mod pool;
pub mod post;
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

fn some_default<T: Default>() -> Option<T> {
    Some(T::default())
}

fn split_into_names(joined_names: &str) -> Vec<SmallString> {
    string::split_unescaped_whitespace(joined_names)
        .map(SmallString::from)
        .collect()
}

fn split_into_ids(joined_ids: &str) -> Result<Vec<i64>, ParseIntError> {
    string::split_unescaped_whitespace(joined_ids)
        .map(|id| id.parse())
        .collect::<Result<_, _>>()
}
