//! Reasons accepted by the reporting API.
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize, ToSchema,
)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "snake_case")]
pub enum ReportReason {
    #[sea_orm(string_value = "suspected_cheating")]
    SuspectedCheating,
    #[sea_orm(string_value = "invalid_lap")]
    InvalidLap,
    #[sea_orm(string_value = "incorrect_classification")]
    IncorrectClassification,
    #[sea_orm(string_value = "other_problem")]
    OtherProblem,
}
