//! `insim_core::vehicle::Vehicle` as a column type.

use insim_core::vehicle::Vehicle;
use sea_orm::{
    ColIdx, DbErr, QueryResult, TryGetError, TryGetable,
    sea_query::{ArrayType, ColumnType, Nullable, StringLen, Value, ValueType, ValueTypeErr},
};

/// A canonical LFS vehicle, standard or mod, stored as its identifier.
///
/// Parsing accepts unknown identifiers as `Vehicle::Unknown`, so the
/// parser must reject that sentinel.
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

/// Parses one stored identifier, rejecting the unknown sentinel.
fn parse(value: &str) -> Option<VehicleId> {
    let vehicle = value
        .parse::<Vehicle>()
        .expect("insim_core vehicle parsing is infallible");
    (vehicle != Vehicle::Unknown).then_some(VehicleId(vehicle))
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

// Catalogue keys are supplied explicitly, never generated from an integer.
impl sea_orm::TryFromU64 for VehicleId {
    fn try_from_u64(_: u64) -> Result<Self, DbErr> {
        Err(DbErr::ConvertFromU64("VehicleId"))
    }
}

impl std::str::FromStr for VehicleId {
    type Err = DbErr;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        parse(value).ok_or_else(|| DbErr::Type(format!("invalid vehicle identifier: {value}")))
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
        // sentinel check rather than by parsing.
        assert!(<VehicleId as ValueType>::try_from(Value::from("")).is_err());
    }
}
