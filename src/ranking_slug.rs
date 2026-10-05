//! Validated public ranking names, scoped to an era.

use sea_orm::{
    ColIdx, DbErr, QueryResult, TryGetError, TryGetable,
    sea_query::{ArrayType, ColumnType, Nullable, Value, ValueType, ValueTypeErr},
};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Stable, URL-safe identity of a concrete ranking within an era.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq)]
#[serde(try_from = "String")]
pub(crate) struct RankingSlug(String);

impl RankingSlug {
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RankingSlug {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Serialize for RankingSlug {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl TryFrom<String> for RankingSlug {
    type Error = RankingSlugError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.split('-').all(|part| {
            !part.is_empty()
                && part
                    .chars()
                    .all(|character| character.is_ascii_lowercase() || character.is_ascii_digit())
        }) {
            Ok(Self(value))
        } else {
            Err(RankingSlugError(value))
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error(
    "ranking slug must contain lowercase ASCII letters or digits separated by single hyphens: {0}"
)]
pub(crate) struct RankingSlugError(String);

impl From<RankingSlug> for Value {
    fn from(value: RankingSlug) -> Self {
        value.0.into()
    }
}

impl TryGetable for RankingSlug {
    fn try_get_by<I: ColIdx>(res: &QueryResult, index: I) -> Result<Self, TryGetError> {
        let value = String::try_get_by(res, index)?;
        <Self as TryFrom<String>>::try_from(value).map_err(|error| {
            TryGetError::DbErr(DbErr::Type(format!("invalid ranking slug stored: {error}")))
        })
    }
}

impl ValueType for RankingSlug {
    fn try_from(v: Value) -> Result<Self, ValueTypeErr> {
        let value = <String as ValueType>::try_from(v)?;
        <Self as TryFrom<String>>::try_from(value).map_err(|_| ValueTypeErr)
    }

    fn type_name() -> String {
        stringify!(RankingSlug).to_owned()
    }

    fn array_type() -> ArrayType {
        ArrayType::String
    }

    fn column_type() -> ColumnType {
        <String as ValueType>::column_type()
    }
}

impl Nullable for RankingSlug {
    fn null() -> Value {
        <String as Nullable>::null()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stored_slugs_follow_the_same_validation_as_configuration() {
        for value in ["all", "single-chart", "s2-2007"] {
            let slug = <RankingSlug as ValueType>::try_from(Value::from(value)).unwrap();
            assert_eq!(slug.as_str(), value);
            assert_eq!(Value::from(slug), Value::from(value));
        }
        for value in ["", "All", "-all", "all-", "all--charts", "all/charts"] {
            assert!(<RankingSlug as ValueType>::try_from(Value::from(value)).is_err());
        }
    }
}
