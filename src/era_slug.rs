//! Validated public names for ranking eras.

use sea_orm::{
    ColIdx, DbErr, QueryResult, TryGetError, TryGetable,
    sea_query::{ArrayType, ColumnType, Nullable, Value, ValueType, ValueTypeErr},
};
use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};
use validator::ValidationError;

/// A non-empty era name that is safe to use as one path component.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(try_from = "String", into = "String")]
pub struct EraSlug(String);

impl EraSlug {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EraSlug {
    type Error = ValidationError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.is_empty()
            || std::path::Path::new(&value).components().count() != 1
            || matches!(value.as_str(), "." | "..")
            || value.contains(std::path::MAIN_SEPARATOR)
        {
            let mut error = ValidationError::new("invalid_era_slug");
            error.message = Some("era slug must be a non-empty, single safe path component".into());
            return Err(error);
        }
        Ok(Self(value))
    }
}

impl FromStr for EraSlug {
    type Err = ValidationError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        value.to_owned().try_into()
    }
}

impl From<EraSlug> for String {
    fn from(value: EraSlug) -> Self {
        value.0
    }
}

impl fmt::Display for EraSlug {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl From<EraSlug> for Value {
    fn from(value: EraSlug) -> Self {
        value.0.into()
    }
}

impl TryGetable for EraSlug {
    fn try_get_by<I: ColIdx>(res: &QueryResult, index: I) -> Result<Self, TryGetError> {
        let value = String::try_get_by(res, index)?;
        <Self as TryFrom<String>>::try_from(value).map_err(|error| {
            TryGetError::DbErr(DbErr::Type(format!("invalid era slug stored: {error}")))
        })
    }
}

impl ValueType for EraSlug {
    fn try_from(v: Value) -> Result<Self, ValueTypeErr> {
        let value = <String as ValueType>::try_from(v)?;
        <Self as TryFrom<String>>::try_from(value).map_err(|_| ValueTypeErr)
    }

    fn type_name() -> String {
        stringify!(EraSlug).to_owned()
    }

    fn array_type() -> ArrayType {
        ArrayType::String
    }

    fn column_type() -> ColumnType {
        <String as ValueType>::column_type()
    }
}

impl Nullable for EraSlug {
    fn null() -> Value {
        <String as Nullable>::null()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn era_slugs_keep_the_existing_single_path_component_rules() {
        for value in [
            "2007-12-21",
            "s2",
            "current-era",
            "era_name",
            "Era Name",
            "é",
        ] {
            let slug = value.parse::<EraSlug>().unwrap();
            assert_eq!(slug.as_str(), value);
            let json = serde_json::to_string(&slug).unwrap();
            assert_eq!(serde_json::from_str::<String>(&json).unwrap(), value);
            assert_eq!(serde_json::from_str::<EraSlug>(&json).unwrap(), slug);
            assert_eq!(
                <EraSlug as ValueType>::try_from(Value::from(value)).unwrap(),
                slug,
            );
        }
        for value in ["", ".", "..", "era/other", "/era", "era/"] {
            assert!(value.parse::<EraSlug>().is_err(), "{value:?}");
            let json = serde_json::to_string(value).unwrap();
            assert!(serde_json::from_str::<EraSlug>(&json).is_err());
            assert!(<EraSlug as ValueType>::try_from(Value::from(value)).is_err());
        }
    }

    #[test]
    fn cli_era_arguments_validate_slugs_before_dispatch() {
        use clap::Parser;

        assert!(
            crate::cli::Args::try_parse_from(["lfsplanet", "era", "export", "2007-12-21"]).is_ok()
        );
        assert!(
            crate::cli::Args::try_parse_from(["lfsplanet", "era", "export", "../other"]).is_err()
        );
        assert!(
            crate::cli::Args::try_parse_from([
                "lfsplanet",
                "hotlap",
                "rebadge",
                "--era",
                "../other"
            ])
            .is_err()
        );
    }
}
