use std::{fmt, str::FromStr};

/// Requested wire protocol, independent of the selected game's era.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, serde::Deserialize)]
pub enum Version {
    #[serde(rename = "1.0")]
    V1_0,
    #[serde(rename = "1.1")]
    V1_1,
    #[serde(rename = "1.2")]
    V1_2,
    #[serde(rename = "1.3")]
    V1_3,
    #[serde(rename = "1.4")]
    V1_4,
    #[default]
    #[serde(rename = "1.5")]
    V1_5,
}

#[derive(Debug, thiserror::Error)]
#[error("Supported Pubstat versions are 1.0 through 1.5")]
pub struct ParseVersionError;

impl FromStr for Version {
    type Err = ParseVersionError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "1.0" => Ok(Self::V1_0),
            "1.1" => Ok(Self::V1_1),
            "1.2" => Ok(Self::V1_2),
            "1.3" => Ok(Self::V1_3),
            "1.4" => Ok(Self::V1_4),
            "1.5" => Ok(Self::V1_5),
            _ => Err(ParseVersionError),
        }
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::V1_0 => "1.0",
            Self::V1_1 => "1.1",
            Self::V1_2 => "1.2",
            Self::V1_3 => "1.3",
            Self::V1_4 => "1.4",
            Self::V1_5 => "1.5",
        })
    }
}
