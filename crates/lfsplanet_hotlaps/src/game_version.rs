//! LFS game versions and version requirements as column types.

use insim_core::game_version::GameVersion;
use sea_orm::{
    ColIdx, DbErr, QueryResult, TryGetError, TryGetable,
    sea_query::{ArrayType, ColumnType, Nullable, StringLen, Value, ValueType, ValueTypeErr},
};
use std::str::FromStr;

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
