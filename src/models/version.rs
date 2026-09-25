//! LFS game versions and version requirements as column types.

use std::str::FromStr;

use insim_core::game_version::GameVersion;
use sea_orm::{
    ColIdx, DbErr, QueryResult, TryGetError, TryGetable,
    sea_query::{ArrayType, ColumnType, Nullable, StringLen, Value, ValueType, ValueTypeErr},
};

/// The LFS version a replay was recorded with, stored as its printed form.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GameVersionCode(pub GameVersion);

/// Versions print as at most `0.5Z28`-style text.
const VERSION_LEN: u32 = 16;

impl From<GameVersion> for GameVersionCode {
    fn from(version: GameVersion) -> Self {
        Self(version)
    }
}

impl std::fmt::Display for GameVersionCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl From<GameVersionCode> for Value {
    fn from(value: GameVersionCode) -> Self {
        value.0.to_string().into()
    }
}

impl TryGetable for GameVersionCode {
    fn try_get_by<I: ColIdx>(res: &QueryResult, index: I) -> Result<Self, TryGetError> {
        let value = String::try_get_by(res, index)?;
        GameVersion::from_str(&value).map(Self).map_err(|error| {
            TryGetError::DbErr(DbErr::Type(format!(
                "invalid game version {value:?} stored: {error}"
            )))
        })
    }
}

impl ValueType for GameVersionCode {
    fn try_from(v: Value) -> Result<Self, ValueTypeErr> {
        let value = <String as ValueType>::try_from(v)?;
        GameVersion::from_str(&value)
            .map(Self)
            .map_err(|_| ValueTypeErr)
    }

    fn type_name() -> String {
        stringify!(GameVersionCode).to_owned()
    }

    fn array_type() -> ArrayType {
        ArrayType::String
    }

    fn column_type() -> ColumnType {
        ColumnType::String(StringLen::N(VERSION_LEN))
    }
}

impl Nullable for GameVersionCode {
    fn null() -> Value {
        <String as Nullable>::null()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lfsplanet_game_version_req::GameVersionReq;

    #[test]
    fn round_trips_a_stored_game_version() {
        let version = GameVersionCode(GameVersion::from_str("0.7G").unwrap());
        assert_eq!(Value::from(version.clone()), Value::from("0.7G"));
        assert_eq!(
            <GameVersionCode as ValueType>::try_from(Value::from("0.7G")).unwrap(),
            version
        );
    }

    #[test]
    fn round_trips_a_stored_version_requirement() {
        let requirement = GameVersionReq::from_str(">=0.5Y,<0.8").unwrap();
        let stored = requirement.to_string();
        assert_eq!(
            <GameVersionReq as ValueType>::try_from(Value::from(stored.as_str())).unwrap(),
            requirement
        );
    }

    #[test]
    fn refuses_requirements_and_versions_that_do_not_parse() {
        assert!(<GameVersionCode as ValueType>::try_from(Value::from("not a version")).is_err());
        assert!(<GameVersionReq as ValueType>::try_from(Value::from(">=nonsense")).is_err());
    }
}
