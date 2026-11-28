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
