//! SeaORM entity for stored player badges by era.

use sea_orm::entity::prelude::*;

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "player_era_badges")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub era_id: i64,
    #[sea_orm(primary_key, auto_increment = false)]
    pub player_id: i64,
    pub badges: Json,
    #[sea_orm(
        belongs_to,
        from = "player_id",
        to = "id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    pub player: BelongsTo<crate::models::players::Entity>,
    #[sea_orm(
        belongs_to,
        from = "era_id",
        to = "id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    pub era: BelongsTo<crate::models::eras::Entity>,
}

impl ActiveModelBehavior for ActiveModel {}
