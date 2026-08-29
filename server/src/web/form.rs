use crate::resource::tag::MicroTag;
use crate::string::SmallString;
use serde::{Deserialize, Deserializer};
use std::collections::BTreeMap;
use std::ops::{Deref, DerefMut};
use std::str::FromStr;

#[derive(Debug, Clone, Copy)]
pub enum TagOperation {
    Save,
    AddImplication,
    AddSuggestion,
    RemoveImplication(usize),
    RemoveSuggestion(usize),
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

#[derive(Debug, Default, PartialEq, Eq, Deserialize)]
pub struct TagMap(BTreeMap<usize, MicroTag>);

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
    type Target = BTreeMap<usize, MicroTag>;
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
        Self(value.into_iter().enumerate().collect())
    }
}
