//! One independently retryable delivery of an event to a subscription.

use crate::models::webhook::{WebhookEvent, WebhookEventKind};

use sea_orm::entity::prelude::*;

use crate::models::{Hotlap, era::Entity as EraEntity, player::Entity as PlayerEntity};

use sea_orm::{
    ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseTransaction, DbBackend, DbErr,
    EntityTrait, QueryFilter, Statement, Value, sea_query::OnConflict,
};

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
        belongs_to = "crate::models::webhook::Entity",
        from = "Column::WebhookId",
        to = "crate::models::webhook::Column::Id"
    )]
    Webhook,
}
impl Related<crate::models::webhook::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Webhook.def()
    }
}
impl ActiveModelBehavior for ActiveModel {}

impl Model {
    /// Snapshot a published hotlap after ranking, within its publication transaction.
    pub(crate) async fn enqueue_hotlap(
        transaction: &DatabaseTransaction,
        hotlap: &Hotlap,
        world_record: bool,
    ) -> Result<(), DbErr> {
        let webhooks = crate::models::webhook::Entity::find()
            .filter(crate::models::webhook::Column::Enabled.eq(true))
            .all(transaction)
            .await?;
        if webhooks.is_empty() {
            return Ok(());
        }
        let player = PlayerEntity::find_by_id(hotlap.player_id)
            .one(transaction)
            .await?
            .ok_or_else(|| DbErr::RecordNotFound("hotlap player".into()))?;
        let era = EraEntity::find_by_id(hotlap.era_id)
            .one(transaction)
            .await?
            .ok_or_else(|| DbErr::RecordNotFound("hotlap era".into()))?;
        let hotlap_id = hotlap.id;
        let driver = player.lfs_username;
        let era = era.slug;
        let track = hotlap.track.to_string();
        let vehicle = hotlap.vehicle.to_string();
        let lap_time_ms = hotlap.lap_time_ms;
        let rank: Option<i64> = transaction
            .query_one_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "SELECT position FROM hotlap_personal_best WHERE hotlap_id = $1",
                [Value::from(hotlap_id)],
            ))
            .await?
            .map(|row| row.try_get("", "position"))
            .transpose()?;
        for webhook in webhooks {
            let event = match webhook.event_kind {
                WebhookEventKind::HotlapValidated => WebhookEvent::HotlapValidated {
                    hotlap_id,
                    driver: driver.clone(),
                    era: era.clone(),
                    track: track.clone(),
                    vehicle: vehicle.clone(),
                    lap_time_ms,
                    rank,
                },
                WebhookEventKind::WorldRecordSet if world_record => WebhookEvent::WorldRecordSet {
                    hotlap_id,
                    driver: driver.clone(),
                    era: era.clone(),
                    track: track.clone(),
                    vehicle: vehicle.clone(),
                    lap_time_ms,
                    rank,
                },
                WebhookEventKind::WorldRecordSet => {
                    continue;
                }
            };
            let kind = WebhookEventKind::from(&event);
            if webhook.event_kind != kind {
                continue;
            }
            Entity::insert(ActiveModel {
                webhook_id: Set(webhook.id),
                hotlap_id: Set(hotlap.id),
                event_kind: Set(kind),
                event: Set(event.clone()),
                ..Default::default()
            })
            .on_conflict(
                OnConflict::columns([Column::WebhookId, Column::HotlapId, Column::EventKind])
                    .do_nothing()
                    .to_owned(),
            )
            .try_insert()
            .exec(transaction)
            .await?;
        }
        Ok(())
    }
}
