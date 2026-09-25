//! SeaORM entity for user-managed personal access tokens.

use sea_orm::entity::prelude::*;

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "personal_access_token")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub player_id: i64,
    pub name: String,
    pub token_hash: Vec<u8>,
    pub token_hint: String,
    pub created_at: TimeDateTimeWithTimeZone,
    pub expires_at: TimeDateTimeWithTimeZone,
    pub last_used_at: Option<TimeDateTimeWithTimeZone>,
    pub revoked_at: Option<TimeDateTimeWithTimeZone>,
    #[sea_orm(
        belongs_to,
        from = "player_id",
        to = "id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    pub player: BelongsTo<crate::models::players::Entity>,
}

impl ActiveModelBehavior for ActiveModel {}
