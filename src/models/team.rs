//! Teams managed by their admin members.

use sea_orm::entity::prelude::*;

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "team")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub is_active: bool,
    pub website_url: Option<String>,
    pub discord_url: Option<String>,
    pub invite_token_hash: Option<Vec<u8>>,
    pub created_at: TimeDateTimeWithTimeZone,
    pub updated_at: TimeDateTimeWithTimeZone,
}

impl ActiveModelBehavior for ActiveModel {}
