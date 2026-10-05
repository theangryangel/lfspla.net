//! A validated name for an LFS installation beneath its configured root.

use sea_orm::{
    ColIdx, DbErr, QueryResult, TryGetError, TryGetable,
    sea_query::{ArrayType, ColumnType, Nullable, Value, ValueType, ValueTypeErr},
};
use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};
use validator::ValidationError;

/// A non-empty, single path component containing lowercase ASCII letters,
/// digits, '.', '_' or '-', excluding '.' and '..'.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Deserialize, Serialize)]
#[serde(try_from = "String", into = "String")]
pub struct InstallationId(String);

impl InstallationId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for InstallationId {
    type Error = ValidationError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.is_empty()
            || matches!(value.as_str(), "." | "..")
            || !value.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"._-".contains(&byte)
            })
        {
            let mut error = ValidationError::new("invalid_installation_id");
            error.message = Some("installation must be a path-safe identifier containing lowercase ASCII letters, digits, '.', '_' or '-'".into());
            return Err(error);
        }
        Ok(Self(value))
    }
}

impl FromStr for InstallationId {
    type Err = ValidationError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        value.to_owned().try_into()
    }
}

impl From<InstallationId> for String {
    fn from(value: InstallationId) -> Self {
        value.0
    }
}

impl fmt::Display for InstallationId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl From<InstallationId> for Value {
    fn from(value: InstallationId) -> Self {
        value.0.into()
    }
}

impl TryGetable for InstallationId {
    fn try_get_by<I: ColIdx>(res: &QueryResult, index: I) -> Result<Self, TryGetError> {
        let value = String::try_get_by(res, index)?;
        <Self as TryFrom<String>>::try_from(value).map_err(|error| {
            TryGetError::DbErr(DbErr::Type(format!(
                "invalid installation identifier stored: {error}"
            )))
        })
    }
}

impl ValueType for InstallationId {
    fn try_from(v: Value) -> Result<Self, ValueTypeErr> {
        let value = <String as ValueType>::try_from(v)?;
        <Self as TryFrom<String>>::try_from(value).map_err(|_| ValueTypeErr)
    }

    fn type_name() -> String {
        stringify!(InstallationId).to_owned()
    }

    fn array_type() -> ArrayType {
        ArrayType::String
    }

    fn column_type() -> ColumnType {
        <String as ValueType>::column_type()
    }
}

impl Nullable for InstallationId {
    fn null() -> Value {
        <String as Nullable>::null()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installation_ids_are_validated_by_cli_parsing() {
        use clap::Parser;

        for value in ["0.7", "../other"] {
            let valid = value == "0.7";
            assert_eq!(
                crate::cli::Args::try_parse_from(["lfsplanet", "lfs", "path", value]).is_ok(),
                valid
            );
            assert_eq!(
                crate::cli::Args::try_parse_from([
                    "lfsplanet",
                    "hotlap",
                    "validate",
                    value,
                    "replay.spr"
                ])
                .is_ok(),
                valid
            );
        }
    }

    #[test]
    fn installation_ids_are_safe_single_path_components() {
        for value in ["0.7", "lfs-0.8", "validator_1"] {
            let id = value.parse::<InstallationId>().unwrap();
            assert_eq!(id.as_str(), value);
            let json = serde_json::to_string(&id).unwrap();
            assert_eq!(serde_json::from_str::<InstallationId>(&json).unwrap(), id);
            assert_eq!(
                <InstallationId as ValueType>::try_from(Value::from(value)).unwrap(),
                id
            );
        }
        for value in ["", ".", "..", "0.7/other", "LFS 0.7", "a\\b", "é", "a\n"] {
            assert!(value.parse::<InstallationId>().is_err(), "{value:?}");
            let json = serde_json::to_string(value).unwrap();
            assert!(serde_json::from_str::<InstallationId>(&json).is_err());
            assert!(<InstallationId as ValueType>::try_from(Value::from(value)).is_err());
        }
    }
}
