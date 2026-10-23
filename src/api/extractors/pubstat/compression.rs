//! Legacy body compression, selected independently of HTTP Accept-Encoding.
use serde::Deserialize;

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
pub(crate) enum Compression {
    #[default]
    #[serde(rename = "0")]
    None,
    #[serde(rename = "1")]
    Gzip,
    #[serde(rename = "2")]
    Zlib,
    #[serde(rename = "3")]
    Deflate,
}
