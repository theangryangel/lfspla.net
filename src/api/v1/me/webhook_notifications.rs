//! Delivery status for the current player's webhook notifications.

use axum::{
    Json,
    extract::{Query, State},
};
use sea_orm::{
    AccessMode, ColumnTrait, EntityTrait, IsolationLevel, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect, RelationTrait, TransactionTrait, sea_query::JoinType,
};
use serde::Serialize;
use time::OffsetDateTime;
use utoipa::ToSchema;

use crate::{
    api::{
        ApiError, ApiState, ErrorResponse, PaginatedResponse, PaginationQuery,
        extractors::BrowserAuthenticatedPlayer,
    },
    models::webhooks::{self, notification},
};

#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
enum DeliveryStatus {
    Pending,
    Delivered,
    Failed,
}

#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct WebhookNotificationResponse {
    id: i64,
    webhook_name: String,
    event_kind: webhooks::WebhookEventKind,
    status: DeliveryStatus,
    attempt_count: i32,
    #[serde(with = "time::serde::rfc3339")]
    #[schema(value_type = String, format = DateTime)]
    created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    #[schema(value_type = Option<String>, format = DateTime, required)]
    next_attempt_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    #[schema(value_type = Option<String>, format = DateTime, required)]
    delivered_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    #[schema(value_type = Option<String>, format = DateTime, required)]
    failed_at: Option<OffsetDateTime>,
    error_detail: Option<String>,
}

#[utoipa::path(
    get,
    path = "/api/v1/me/webhooks/notifications",
    operation_id = "list_webhook_notifications",
    tag = "webhooks",
    security(("cookie_session" = [])),
    params(PaginationQuery),
    responses(
        (status = 200, description = "Current player's webhook delivery status", body = PaginatedResponse<WebhookNotificationResponse>),
        (status = 400, body = ErrorResponse),
        (status = 401, body = ErrorResponse)
    )
)]
pub(crate) async fn list(
    Query(pagination): Query<PaginationQuery>,
    State(state): State<ApiState>,
    BrowserAuthenticatedPlayer(player): BrowserAuthenticatedPlayer,
) -> Result<Json<PaginatedResponse<WebhookNotificationResponse>>, ApiError> {
    let offset = pagination.offset()?;
    let transaction = state
        .database
        .begin_with_config(
            Some(IsolationLevel::RepeatableRead),
            Some(AccessMode::ReadOnly),
        )
        .await
        .map_err(ApiError::database)?;
    let notifications = notification::Entity::find()
        .join(JoinType::InnerJoin, notification::Relation::Webhook.def())
        .filter(webhooks::Column::PlayerId.eq(player.id))
        .order_by_desc(notification::Column::CreatedAt)
        .order_by_desc(notification::Column::Id);
    let total = notifications
        .clone()
        .count(&transaction)
        .await
        .map_err(ApiError::database)?;
    let rows = notifications
        .offset(offset)
        .limit(pagination.per_page)
        .all(&transaction)
        .await
        .map_err(ApiError::database)?;
    let webhook_ids: Vec<i64> = rows.iter().map(|row| row.webhook_id).collect();
    let webhook_names = if webhook_ids.is_empty() {
        Vec::new()
    } else {
        webhooks::Entity::find()
            .filter(webhooks::Column::Id.is_in(webhook_ids))
            .all(&transaction)
            .await
            .map_err(ApiError::database)?
    };
    let webhook_names: std::collections::HashMap<_, _> = webhook_names
        .into_iter()
        .map(|webhook| (webhook.id, webhook.name))
        .collect();
    let items = rows
        .into_iter()
        .map(|notification| {
            let webhook_name = webhook_names
                .get(&notification.webhook_id)
                .cloned()
                .ok_or_else(|| {
                    ApiError::internal(&notification.webhook_id, "webhook notification destination")
                })?;
            let status = if notification.delivered_at.is_some() {
                DeliveryStatus::Delivered
            } else if notification.failed_at.is_some() {
                DeliveryStatus::Failed
            } else {
                DeliveryStatus::Pending
            };
            Ok(WebhookNotificationResponse {
                id: notification.id,
                webhook_name,
                event_kind: notification.event_kind,
                status,
                attempt_count: notification.attempt_count,
                created_at: notification.created_at,
                next_attempt_at: if matches!(status, DeliveryStatus::Pending) {
                    Some(notification.next_attempt_at)
                } else {
                    None
                },
                delivered_at: notification.delivered_at,
                failed_at: notification.failed_at,
                error_detail: notification.error_detail,
            })
        })
        .collect::<Result<Vec<_>, ApiError>>()?;
    transaction.commit().await.map_err(ApiError::database)?;
    Ok(Json(PaginatedResponse {
        items,
        pagination: pagination.metadata(total),
    }))
}
