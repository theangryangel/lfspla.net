//! User-owned webhook subscriptions. URLs contain credentials; never log models.

use super::{WebhookEventKind, WebhookFormat};
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "webhook")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub player_id: i64,
    pub name: String,
    pub url: String,
    pub format: WebhookFormat,
    pub event_kind: WebhookEventKind,
    pub enabled: bool,
    pub available_at: TimeDateTimeWithTimeZone,
    pub created_at: TimeDateTimeWithTimeZone,
}
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}
