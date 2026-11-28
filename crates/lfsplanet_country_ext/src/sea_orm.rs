use sea_orm::{
    ColIdx, DbErr, QueryResult, TryGetError, TryGetable,
    sea_query::{ArrayType, ColumnType, Nullable, Value, ValueType, ValueTypeErr},
};

use crate::FlagCode;

impl From<FlagCode> for Value {
    fn from(code: FlagCode) -> Self {
        code.as_str().into()
    }
}

impl TryGetable for FlagCode {
    fn try_get_by<I: ColIdx>(res: &QueryResult, index: I) -> Result<Self, TryGetError> {
        let value = String::try_get_by(res, index)?;
        Self::parse(&value).ok_or_else(|| {
            TryGetError::DbErr(DbErr::Type(format!("invalid stored flag code: {value:?}")))
        })
    }
}

impl ValueType for FlagCode {
    fn try_from(value: Value) -> Result<Self, ValueTypeErr> {
        let value = <String as ValueType>::try_from(value)?;
        Self::parse(&value).ok_or(ValueTypeErr)
    }

    fn type_name() -> String {
        stringify!(FlagCode).to_owned()
    }

    fn array_type() -> ArrayType {
        ArrayType::String
    }

    fn column_type() -> ColumnType {
        ColumnType::Text
    }
}

impl Nullable for FlagCode {
    fn null() -> Value {
        <String as Nullable>::null()
    }
}
