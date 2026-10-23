//! Pubstat's numeric output selector, deserialized directly from query values.
use serde::Deserialize;

#[derive(Clone, Copy, Default, Deserialize, PartialEq, Eq)]
pub(crate) enum Format {
    #[default]
    #[serde(rename = "0")]
    Text,
    #[serde(rename = "1")]
    Json,
    #[serde(rename = "2")]
    Php,
    #[serde(rename = "3")]
    Xml,
}
