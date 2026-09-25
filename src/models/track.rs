//! `insim_core::track::Track` as a column type.

use std::str::FromStr;

use insim_core::track::Track;
use sea_orm::{
    ColIdx, DbErr, QueryResult, TryGetError, TryGetable,
    sea_query::{ArrayType, ColumnType, Nullable, StringLen, Value, ValueType, ValueTypeErr},
};

/// A canonical LFS track configuration, stored as its short code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TrackId(pub Track);

/// Track codes are at most six characters (`BL1R`, `WE1XR`).
const TRACK_CODE_LEN: u32 = 8;

impl From<Track> for TrackId {
    fn from(track: Track) -> Self {
        Self(track)
    }
}

impl std::fmt::Display for TrackId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl From<TrackId> for Value {
    fn from(value: TrackId) -> Self {
        value.0.to_string().into()
    }
}

impl TryGetable for TrackId {
    fn try_get_by<I: ColIdx>(res: &QueryResult, index: I) -> Result<Self, TryGetError> {
        let value = String::try_get_by(res, index)?;
        Track::from_str(&value).map(Self).map_err(|error| {
            TryGetError::DbErr(DbErr::Type(format!(
                "invalid track {value:?} stored: {error}"
            )))
        })
    }
}

impl ValueType for TrackId {
    fn try_from(v: Value) -> Result<Self, ValueTypeErr> {
        let value = <String as ValueType>::try_from(v)?;
        Track::from_str(&value).map(Self).map_err(|_| ValueTypeErr)
    }

    fn type_name() -> String {
        stringify!(TrackId).to_owned()
    }

    fn array_type() -> ArrayType {
        ArrayType::String
    }

    fn column_type() -> ColumnType {
        ColumnType::String(StringLen::N(TRACK_CODE_LEN))
    }
}

impl Nullable for TrackId {
    fn null() -> Value {
        <String as Nullable>::null()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_the_stored_track_code() {
        let track = TrackId(Track::So4r);
        assert_eq!(Value::from(track), Value::from("SO4R"));
        assert_eq!(
            <TrackId as ValueType>::try_from(Value::from("SO4R")).unwrap(),
            track
        );
    }

    #[test]
    fn refuses_a_code_that_names_no_track() {
        assert!(<TrackId as ValueType>::try_from(Value::from("ZZ9Z")).is_err());
    }
}
