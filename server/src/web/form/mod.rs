use crate::resource::tag::MicroTag;
use crate::string::SmallString;
use serde::{Deserialize, Deserializer};
use std::collections::BTreeMap;
use std::ops::{Deref, DerefMut};

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

#[derive(Default, PartialEq, Eq, Deserialize)]
pub struct TagMap(BTreeMap<i64, MicroTag>);

impl TagMap {
    pub fn names(&self) -> Vec<SmallString> {
        self.0
            .values()
            .map(MicroTag::primary_name)
            .map(SmallString::from)
            .collect()
    }

    fn append_tags(&mut self, tags: Vec<MicroTag>) {
        let lowest_current_index = self.first_key_value().map_or(0, |(lowest_index, _)| *lowest_index);
        for (offset, micro_tag) in (1..).zip(tags) {
            self.insert(lowest_current_index - offset, micro_tag);
        }
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

impl<'a> IntoIterator for &'a TagMap {
    type Item = (&'a i64, &'a MicroTag);
    type IntoIter = std::collections::btree_map::Iter<'a, i64, MicroTag>;
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
