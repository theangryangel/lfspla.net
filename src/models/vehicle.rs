//! `insim_core::vehicle::Vehicle` as a column type.

use insim_core::vehicle::Vehicle;
use sea_orm::{
    ColIdx, DbErr, QueryResult, TryGetError, TryGetable,
    sea_query::{ArrayType, ColumnType, Nullable, StringLen, Value, ValueType, ValueTypeErr},
};

use crate::models::hotlaps::HotlapRankable;

/// A canonical LFS vehicle, standard or mod, stored as its identifier.
///
/// Parsing accepts unknown identifiers as `Vehicle::Unknown`, so the
/// rankability check must reject them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VehicleId(pub Vehicle);

/// Mod identifiers are six hex characters; standard codes are three.
const VEHICLE_ID_LEN: u32 = 8;

impl std::fmt::Display for VehicleId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl From<VehicleId> for Value {
    fn from(value: VehicleId) -> Self {
        value.0.to_string().into()
    }
}

/// Parses and rankability-checks one stored identifier.
fn parse(value: &str) -> Option<VehicleId> {
    value
        .parse::<Vehicle>()
        .expect("insim_core vehicle parsing is infallible")
        .ensure_hotlap_rankable()
        .ok()
        .map(VehicleId)
}

impl TryGetable for VehicleId {
    fn try_get_by<I: ColIdx>(res: &QueryResult, index: I) -> Result<Self, TryGetError> {
        let value = String::try_get_by(res, index)?;
        parse(&value).ok_or_else(|| {
            TryGetError::DbErr(DbErr::Type(format!(
                "invalid vehicle {value:?} stored: identifier cannot participate in a ranked hotlap"
            )))
        })
    }
}

impl ValueType for VehicleId {
    fn try_from(v: Value) -> Result<Self, ValueTypeErr> {
        let value = <String as ValueType>::try_from(v)?;
        parse(&value).ok_or(ValueTypeErr)
    }

    fn type_name() -> String {
        stringify!(VehicleId).to_owned()
    }

    fn array_type() -> ArrayType {
        ArrayType::String
    }

    fn column_type() -> ColumnType {
        ColumnType::String(StringLen::N(VEHICLE_ID_LEN))
    }
}

impl Nullable for VehicleId {
    fn null() -> Value {
        <String as Nullable>::null()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_standard_and_mod_identifiers() {
        assert_eq!(Value::from(VehicleId(Vehicle::Rb4)), Value::from("RB4"));
        let modded = <VehicleId as ValueType>::try_from(Value::from("728419")).unwrap();
        assert_eq!(Value::from(modded), Value::from("728419"));
    }

    #[test]
    fn refuses_an_identifier_that_parses_only_as_unknown() {
        // `Vehicle::from_str` is infallible, so this is rejected by the
        // rankability check rather than by parsing.
        assert!(<VehicleId as ValueType>::try_from(Value::from("")).is_err());
    }
}
