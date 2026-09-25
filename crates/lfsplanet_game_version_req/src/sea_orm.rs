//! SeaORM integration for [`super::GameVersionReq`].

use std::str::FromStr;

use sea_orm::{
    ColIdx, DbErr, QueryResult, TryGetError, TryGetable,
    sea_query::{ArrayType, ColumnType, Nullable, StringLen, Value, ValueType, ValueTypeErr},
};

use super::GameVersionReq;

impl From<GameVersionReq> for Value {
    fn from(value: GameVersionReq) -> Self {
        value.to_string().into()
    }
}

impl TryGetable for GameVersionReq {
    fn try_get_by<I: ColIdx>(res: &QueryResult, index: I) -> Result<Self, TryGetError> {
        let value = String::try_get_by(res, index)?;
        Self::from_str(&value).map_err(|error| {
            TryGetError::DbErr(DbErr::Type(format!(
                "invalid version requirement {value:?} stored: {error}"
            )))
        })
    }
}

impl ValueType for GameVersionReq {
    fn try_from(v: Value) -> Result<Self, ValueTypeErr> {
        let value = <String as ValueType>::try_from(v)?;
        Self::from_str(&value).map_err(|_| ValueTypeErr)
    }

    fn type_name() -> String {
        stringify!(GameVersionReq).to_owned()
    }

    fn array_type() -> ArrayType {
        ArrayType::String
    }

    fn column_type() -> ColumnType {
        ColumnType::String(StringLen::None)
    }
}

impl Nullable for GameVersionReq {
    fn null() -> Value {
        <String as Nullable>::null()
    }
}
