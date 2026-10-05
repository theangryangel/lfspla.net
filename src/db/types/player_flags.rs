//! LFS player flags as a column type.

use lfsplanet_spr::PlayerFlags;
use sea_orm::{
    ColIdx, DbErr, QueryResult, TryGetError, TryGetable,
    sea_query::{ArrayType, ColumnType, Nullable, Value, ValueType, ValueTypeErr},
};

/// The raw 16-bit player flags exactly as LFS wrote them.
///
/// Stored as `INTEGER` because PostgreSQL has no unsigned 16-bit type.
/// Values outside the 16-bit range are rejected on read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayerFlagsBits(pub PlayerFlags);

impl From<PlayerFlags> for PlayerFlagsBits {
    fn from(flags: PlayerFlags) -> Self {
        Self(flags)
    }
}

impl From<PlayerFlagsBits> for Value {
    fn from(value: PlayerFlagsBits) -> Self {
        i32::from(value.0.bits()).into()
    }
}

fn from_stored(value: i32) -> Option<PlayerFlagsBits> {
    <u16 as TryFrom<i32>>::try_from(value)
        .ok()
        .map(|bits| PlayerFlagsBits(PlayerFlags::from_bits_retain(bits)))
}

impl TryGetable for PlayerFlagsBits {
    fn try_get_by<I: ColIdx>(res: &QueryResult, index: I) -> Result<Self, TryGetError> {
        let value = i32::try_get_by(res, index)?;
        from_stored(value).ok_or_else(|| {
            TryGetError::DbErr(DbErr::Type(format!(
                "player flags {value} stored do not fit 16 bits"
            )))
        })
    }
}

impl ValueType for PlayerFlagsBits {
    fn try_from(v: Value) -> Result<Self, ValueTypeErr> {
        let value = <i32 as ValueType>::try_from(v)?;
        from_stored(value).ok_or(ValueTypeErr)
    }

    fn type_name() -> String {
        stringify!(PlayerFlagsBits).to_owned()
    }

    fn array_type() -> ArrayType {
        ArrayType::Int
    }

    fn column_type() -> ColumnType {
        ColumnType::Integer
    }
}

impl Nullable for PlayerFlagsBits {
    fn null() -> Value {
        <i32 as Nullable>::null()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retains_every_bit_including_ones_this_build_cannot_name() {
        // Bits 1 and 2 have no name in the current protocol model.
        let bits: u16 = 1 | 2 | 4 | 8 | 512 | 1024;
        let stored = i32::from(bits);
        let flags = <PlayerFlagsBits as ValueType>::try_from(Value::from(stored)).unwrap();
        assert_eq!(flags.0.bits(), bits);
        assert_eq!(Value::from(flags), Value::from(stored));
    }

    #[test]
    fn refuses_stored_flags_that_do_not_fit_sixteen_bits() {
        assert!(<PlayerFlagsBits as ValueType>::try_from(Value::from(i32::MAX)).is_err());
        assert!(<PlayerFlagsBits as ValueType>::try_from(Value::from(-1_i32)).is_err());
    }
}
