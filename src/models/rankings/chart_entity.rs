//! Ordered chart selection owned directly by one ranking.
#![allow(
    clippy::struct_field_names,
    reason = "entity fields mirror database column names"
)]

use sea_orm::entity::prelude::*;

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "ranking_chart")]
pub struct Model {
    pub era_id: i64,
    #[sea_orm(primary_key, auto_increment = false)]
    pub ranking_id: i64,
    #[sea_orm(primary_key, auto_increment = false)]
    pub position: i32,
    pub track_id: String,
    pub vehicle_id: String,
    #[sea_orm(
        belongs_to,
        from = "ranking_id",
        to = "id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    pub ranking: BelongsTo<crate::models::rankings::Entity>,
    #[sea_orm(belongs_to, from = "track_id", to = "id")]
    pub track: BelongsTo<crate::models::tracks::Entity>,
    #[sea_orm(belongs_to, from = "vehicle_id", to = "id")]
    pub vehicle: BelongsTo<crate::models::vehicles::Entity>,
}

impl ActiveModelBehavior for ActiveModel {}
