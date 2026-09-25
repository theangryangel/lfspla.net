//! One independently retryable delivery of an event to a subscription.

use super::{WebhookEvent, WebhookEventKind};
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "webhook_notification")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub webhook_id: i64,
    pub hotlap_id: i64,
    pub event_kind: WebhookEventKind,
    #[sea_orm(column_type = "JsonBinary")]
    pub event: WebhookEvent,
    pub attempt_count: i32,
    pub next_attempt_at: TimeDateTimeWithTimeZone,
    pub delivered_at: Option<TimeDateTimeWithTimeZone>,
    pub failed_at: Option<TimeDateTimeWithTimeZone>,
    pub error_detail: Option<String>,
    pub created_at: TimeDateTimeWithTimeZone,
}
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::entity::Entity",
        from = "Column::WebhookId",
        to = "super::entity::Column::Id"
    )]
    Webhook,
}
impl Related<super::entity::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Webhook.def()
    }
}
impl ActiveModelBehavior for ActiveModel {}
