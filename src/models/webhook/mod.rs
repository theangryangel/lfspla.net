//! Webhook subscriptions and their ownership-scoped persistence operations.

use sea_orm::{
    ActiveValue::Set, ColumnTrait, ConnectionTrait, DbErr, DeriveActiveEnum, EntityTrait, EnumIter,
    QueryFilter,
};

use serde::{Deserialize, Serialize};

use utoipa::ToSchema;

use sea_orm::entity::prelude::*;

pub(crate) mod discord;
mod event;

pub use event::WebhookEvent;
pub use event::WebhookEventKind;

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

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize, ToSchema,
)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "snake_case")]
pub enum WebhookFormat {
    #[sea_orm(string_value = "discord")]
    Discord,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}

/// Validated attributes for a new subscription.
pub(crate) struct NewWebhook {
    pub(crate) player_id: i64,
    pub(crate) name: String,
    pub(crate) url: String,
    pub(crate) format: WebhookFormat,
    pub(crate) event_kind: WebhookEventKind,
}
impl Model {
    pub(crate) async fn list_for_player(
        database: &impl ConnectionTrait,
        player_id: i64,
    ) -> Result<Vec<Self>, DbErr> {
        use sea_orm::QueryOrder;
        Entity::find()
            .filter(Column::PlayerId.eq(player_id))
            .order_by_desc(Column::Id)
            .all(database)
            .await
    }
    pub(crate) async fn create(
        database: &impl ConnectionTrait,
        new: NewWebhook,
    ) -> Result<Self, DbErr> {
        use sea_orm::ActiveModelTrait;
        ActiveModel {
            player_id: Set(new.player_id),
            name: Set(new.name),
            url: Set(new.url),
            format: Set(new.format),
            event_kind: Set(new.event_kind),
            ..Default::default()
        }
        .insert(database)
        .await
    }
    /// Ownership is part of the update predicate.
    pub(crate) async fn set_enabled_owned(
        database: &impl ConnectionTrait,
        id: i64,
        player_id: i64,
        enabled: bool,
    ) -> Result<Option<Self>, DbErr> {
        Ok(Entity::update_many()
            .col_expr(Column::Enabled, sea_orm::sea_query::Expr::value(enabled))
            .filter(Column::Id.eq(id))
            .filter(Column::PlayerId.eq(player_id))
            .exec_with_returning(database)
            .await?
            .into_iter()
            .next())
    }
    pub(crate) async fn delete_owned(
        database: &impl ConnectionTrait,
        id: i64,
        player_id: i64,
    ) -> Result<bool, DbErr> {
        Ok(Entity::delete_many()
            .filter(Column::Id.eq(id))
            .filter(Column::PlayerId.eq(player_id))
            .exec(database)
            .await?
            .rows_affected
            > 0)
    }
}
