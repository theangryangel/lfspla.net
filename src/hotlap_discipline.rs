//! Discipline used to group rankings without separating their chart system.

use sea_orm::{DeriveActiveEnum, EnumIter};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// A ranking's racing discipline. Stored as text and serialized in snake case.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    PartialEq,
    EnumIter,
    DeriveActiveEnum,
    Serialize,
    Deserialize,
    ToSchema,
)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "snake_case")]
pub enum HotLapDiscipline {
    #[default]
    #[sea_orm(string_value = "circuit")]
    Circuit,
    #[sea_orm(string_value = "drag")]
    Drag,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serde_values_are_explicit_and_unknown_disciplines_are_rejected() {
        for (discipline, value) in [
            (HotLapDiscipline::Circuit, "circuit"),
            (HotLapDiscipline::Drag, "drag"),
        ] {
            assert_eq!(serde_json::to_value(discipline).unwrap(), value);
            assert_eq!(
                serde_json::from_value::<HotLapDiscipline>(serde_json::json!(value)).unwrap(),
                discipline
            );
        }
        assert!(serde_json::from_str::<HotLapDiscipline>("\"drift\"").is_err());
    }
}
