//! `celes::Country` as a column type.

use celes::Country;
use sea_orm::{
    ColIdx, DbErr, QueryResult, TryGetError, TryGetable,
    sea_query::{ArrayType, ColumnType, Nullable, StringLen, Value, ValueType, ValueTypeErr},
};
use serde::{Serialize, Serializer};

/// An ISO 3166-1 country, stored as its alpha-2 code.
///
/// Invalid country codes are rejected when reading rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CountryCode(pub Country);

impl CountryCode {
    /// The alpha-2 code, as stored and as sent over the wire.
    pub fn as_str(&self) -> &'static str {
        self.0.alpha2
    }
}

impl From<Country> for CountryCode {
    fn from(country: Country) -> Self {
        Self(country)
    }
}

/// Serializes as the alpha-2 code.
impl Serialize for CountryCode {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl From<CountryCode> for Value {
    fn from(value: CountryCode) -> Self {
        value.as_str().into()
    }
}

impl TryGetable for CountryCode {
    fn try_get_by<I: ColIdx>(res: &QueryResult, index: I) -> Result<Self, TryGetError> {
        let value = String::try_get_by(res, index)?;
        Country::from_alpha2(&value).map(Self).map_err(|error| {
            TryGetError::DbErr(DbErr::Type(format!(
                "invalid country code {value:?} stored: {error}"
            )))
        })
    }
}

impl ValueType for CountryCode {
    fn try_from(v: Value) -> Result<Self, ValueTypeErr> {
        let value = <String as ValueType>::try_from(v)?;
        Country::from_alpha2(&value)
            .map(Self)
            .map_err(|_| ValueTypeErr)
    }

    fn type_name() -> String {
        stringify!(CountryCode).to_owned()
    }

    fn array_type() -> ArrayType {
        ArrayType::String
    }

    fn column_type() -> ColumnType {
        ColumnType::String(StringLen::N(2))
    }
}

impl Nullable for CountryCode {
    fn null() -> Value {
        <String as Nullable>::null()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_the_stored_alpha2_code() {
        let code = CountryCode(Country::the_united_kingdom_of_great_britain_and_northern_ireland());
        assert_eq!(code.as_str(), "GB");
        assert_eq!(Value::from(code), Value::from("GB"));
        assert_eq!(
            <CountryCode as ValueType>::try_from(Value::from("GB")).unwrap(),
            code
        );
    }

    #[test]
    fn refuses_a_code_that_names_no_country() {
        assert!(<CountryCode as ValueType>::try_from(Value::from("XX")).is_err());
    }

    #[test]
    fn serializes_as_the_bare_alpha2_code() {
        let code = CountryCode(Country::germany());
        assert_eq!(serde_json::to_value(code).unwrap(), "DE");
    }
}
