use crate::time::DateTime;
use serde::Deserialize;

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Operation {
    Write,
    Preview,
    Save,
    Cancel,
}

#[derive(Deserialize)]
pub struct EditForm {
    pub operation: Operation,
    pub version: DateTime,
    pub text: String,
}
