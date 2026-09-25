//! Webhook subscriptions, event snapshots, and destination-specific rendering.

pub(crate) mod discord;
mod entity;
mod event;
pub(crate) mod notification;

pub use entity::{ActiveModel, Column, Entity, Model};
pub use event::{WebhookEvent, WebhookEventKind};

use sea_orm::{
    ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseTransaction, DbBackend, DbErr,
    DeriveActiveEnum, EntityTrait, EnumIter, QueryFilter, Statement, Value, sea_query::OnConflict,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::models::{eras::EraEntity, hotlaps::HotlapModel, players::Entity as PlayerEntity};

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize, ToSchema,
)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "snake_case")]
pub enum WebhookFormat {
    #[sea_orm(string_value = "discord")]
    Discord,
}

/// Called after publication and ranking, using the same transaction.
/// Snapshots remain stable if driver or catalogue names subsequently change.
pub async fn enqueue_hotlap(
    transaction: &DatabaseTransaction,
    hotlap: &HotlapModel,
    world_record: bool,
) -> Result<(), DbErr> {
    let webhooks = Entity::find()
        .filter(Column::Enabled.eq(true))
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
    let vehicle = hotlap
        .vehicle
        .ok_or_else(|| DbErr::Type("valid hotlap has no vehicle".into()))?
        .to_string();
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
        notification::Entity::insert(notification::ActiveModel {
            webhook_id: Set(webhook.id),
            hotlap_id: Set(hotlap.id),
            event_kind: Set(kind),
            event: Set(event.clone()),
            ..Default::default()
        })
        .on_conflict(
            OnConflict::columns([
                notification::Column::WebhookId,
                notification::Column::HotlapId,
                notification::Column::EventKind,
            ])
            .do_nothing()
            .to_owned(),
        )
        .try_insert()
        .exec(transaction)
        .await?;
    }
    Ok(())
}
