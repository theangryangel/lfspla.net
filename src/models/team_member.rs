//! Membership of multiple teams, with roles stored as ordinary text.

use sea_orm::{DeriveActiveEnum, EnumIter, entity::prelude::*};
use serde::{Deserialize, Serialize};

/// All admins have equal access; an active team must retain at least one.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    EnumIter,
    DeriveActiveEnum,
    Serialize,
    Deserialize,
    utoipa::ToSchema,
)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "snake_case")]
pub enum Role {
    #[default]
    #[sea_orm(string_value = "member")]
    Member,
    #[sea_orm(string_value = "admin")]
    Admin,
}

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "team_member")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub team_id: i64,
    #[sea_orm(primary_key, auto_increment = false)]
    pub player_id: i64,
    pub role: Role,
    pub joined_at: TimeDateTimeWithTimeZone,
}

impl ActiveModelBehavior for ActiveModel {}
