//! Team membership also determines a player's permissions within that team.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "team_member")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub team_id: i64,
    #[sea_orm(primary_key, auto_increment = false)]
    pub player_id: i64,
    pub role: Role,
    #[sea_orm(belongs_to, from = "team_id", to = "id", on_delete = "Cascade")]
    pub team: BelongsTo<super::team::Entity>,
    #[sea_orm(belongs_to, from = "player_id", to = "id")]
    pub player: BelongsTo<super::player::Entity>,
}

impl ActiveModelBehavior for ActiveModel {}

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
    ToSchema,
)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "snake_case")]
pub enum Role {
    #[sea_orm(string_value = "owner")]
    Owner,
    #[default]
    #[sea_orm(string_value = "member")]
    Member,
}
