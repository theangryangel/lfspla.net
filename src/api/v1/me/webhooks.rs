//! Browser-managed webhook subscriptions; destination credentials are write-only.

use crate::{
    api::{
        ApiError, ApiState, ErrorResponse, ListResponse, extractors::BrowserAuthenticatedPlayer,
    },
    models::webhook::{self, WebhookEventKind, WebhookFormat},
};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};

use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;
use utoipa::ToSchema;

/// Available choices come from the same enums accepted by creation.
#[derive(Serialize, ToSchema)]
pub(crate) struct WebhookOptionsResponse {
    formats: Vec<WebhookFormatOption>,
    events: Vec<WebhookEventOption>,
}

#[derive(Serialize, ToSchema)]
struct WebhookFormatOption {
    value: WebhookFormat,
    label: &'static str,
    url_placeholder: &'static str,
    url_help: &'static str,
}

impl From<WebhookFormat> for WebhookFormatOption {
    fn from(value: WebhookFormat) -> Self {
        match value {
            WebhookFormat::Discord => Self {
                value,
                label: "Discord",
                url_placeholder: "https://discord.com/api/webhooks/...",
                url_help: "In Discord, open your channel’s settings, then Integrations → Webhooks. Create a webhook and copy its URL here.",
            },
        }
    }
}

#[derive(Serialize, ToSchema)]
struct WebhookEventOption {
    value: WebhookEventKind,
    label: &'static str,
    description: &'static str,
}

impl From<WebhookEventKind> for WebhookEventOption {
    fn from(value: WebhookEventKind) -> Self {
        match value {
            WebhookEventKind::HotlapValidated => Self {
                value,
                label: "Hotlap validated",
                description: "Every successfully validated hotlap, from all drivers, including laps that do not improve a personal best.",
            },
            WebhookEventKind::WorldRecordSet => Self {
                value,
                label: "World record set",
                description: "When a validated hotlap becomes first on its chart.",
            },
        }
    }
}

#[utoipa::path(get, operation_id = "webhook_options", path = "/api/v1/me/webhooks/options", tag = "webhooks", security(("cookie_session" = [])),
    responses((status = 200, description = "Webhook configuration options", body = WebhookOptionsResponse), (status = 401, description = "Browser authentication required", body = ErrorResponse)))]
pub(crate) async fn options(
    BrowserAuthenticatedPlayer(_player): BrowserAuthenticatedPlayer,
) -> Json<WebhookOptionsResponse> {
    Json(WebhookOptionsResponse {
        formats: WebhookFormat::iter().map(Into::into).collect(),
        events: WebhookEventKind::iter().map(Into::into).collect(),
    })
}

#[derive(Deserialize, ToSchema)]
pub(crate) struct CreateWebhookRequest {
    name: String,
    url: String,
    format: WebhookFormat,
    event_kind: WebhookEventKind,
}
#[derive(Deserialize, ToSchema)]
pub(crate) struct UpdateWebhookRequest {
    enabled: bool,
}
#[derive(Serialize, ToSchema)]
pub(crate) struct WebhookResponse {
    id: i64,
    name: String,
    format: WebhookFormat,
    event_kind: WebhookEventKind,
    enabled: bool,
}
impl From<crate::models::Webhook> for WebhookResponse {
    fn from(model: crate::models::Webhook) -> Self {
        Self {
            id: model.id,
            name: model.name,
            format: model.format,
            event_kind: model.event_kind,
            enabled: model.enabled,
        }
    }
}

#[utoipa::path(get, operation_id = "list_webhooks", path = "/api/v1/me/webhooks", tag = "webhooks", security(("cookie_session" = [])),
    responses((status = 200, description = "Configured webhooks", body = ListResponse<WebhookResponse>), (status = 401, description = "Browser authentication required", body = ErrorResponse)))]
pub(crate) async fn list(
    State(state): State<ApiState>,
    BrowserAuthenticatedPlayer(player): BrowserAuthenticatedPlayer,
) -> Result<Json<ListResponse<WebhookResponse>>, ApiError> {
    let records = crate::models::Webhook::list_for_player(&state.database, player.id)
        .await
        .map_err(ApiError::database)?;
    Ok(Json(ListResponse::from(
        records.into_iter().map(Into::into).collect::<Vec<_>>(),
    )))
}

#[utoipa::path(post, operation_id = "create_webhook", path = "/api/v1/me/webhooks", tag = "webhooks", security(("cookie_session" = [])),
    params(("X-CSRF-Token" = String, Header)), request_body = CreateWebhookRequest,
    responses((status = 201, description = "Webhook created", body = WebhookResponse), (status = 400, description = "Webhook details are invalid", body = ErrorResponse), (status = 401, description = "Browser authentication required", body = ErrorResponse), (status = 403, description = "CSRF token missing or invalid", body = ErrorResponse), (status = 409, description = "Webhook already exists", body = ErrorResponse)))]
pub(crate) async fn create(
    State(state): State<ApiState>,
    BrowserAuthenticatedPlayer(player): BrowserAuthenticatedPlayer,
    Json(request): Json<CreateWebhookRequest>,
) -> Result<(StatusCode, Json<WebhookResponse>), ApiError> {
    let name = request.name.trim();
    if name.is_empty() || name.chars().count() > 100 || request.name.chars().any(char::is_control) {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_webhook_name",
            "Use a webhook name between 1 and 100 characters without control characters",
        ));
    }
    let url = match request.format {
        WebhookFormat::Discord => crate::models::webhook::discord::webhook_url(&request.url)
            .map_err(|message| {
                ApiError::new(StatusCode::BAD_REQUEST, "invalid_webhook_url", message)
            })?,
    };
    let result = crate::models::Webhook::create(
        &state.database,
        webhook::NewWebhook {
            player_id: player.id,
            name: name.into(),
            url: url.into(),
            format: request.format,
            event_kind: request.event_kind,
        },
    )
    .await
    .map_err(|error| {
        if matches!(
            error.sql_err(),
            Some(sea_orm::SqlErr::UniqueConstraintViolation(_))
        ) {
            ApiError::new(
                StatusCode::CONFLICT,
                "webhook_exists",
                "This webhook is already subscribed to this event",
            )
        } else {
            ApiError::database(error)
        }
    })?;
    Ok((StatusCode::CREATED, Json(result.into())))
}

#[utoipa::path(patch, operation_id = "update_webhook", path = "/api/v1/me/webhooks/{webhook}", tag = "webhooks", security(("cookie_session" = [])),
    params(("webhook" = i64, Path), ("X-CSRF-Token" = String, Header)), request_body = UpdateWebhookRequest,
    responses((status = 200, description = "Webhook updated", body = WebhookResponse), (status = 401, description = "Browser authentication required", body = ErrorResponse), (status = 403, description = "CSRF token missing or invalid", body = ErrorResponse), (status = 404, description = "Webhook not found", body = ErrorResponse)))]
pub(crate) async fn update(
    Path(id): Path<i64>,
    State(state): State<ApiState>,
    BrowserAuthenticatedPlayer(player): BrowserAuthenticatedPlayer,
    Json(request): Json<UpdateWebhookRequest>,
) -> Result<Json<WebhookResponse>, ApiError> {
    let record =
        crate::models::Webhook::set_enabled_owned(&state.database, id, player.id, request.enabled)
            .await
            .map_err(ApiError::database)?
            .ok_or_else(|| ApiError::not_found("webhook_not_found", "Webhook"))?;
    Ok(Json(record.into()))
}

#[utoipa::path(delete, operation_id = "delete_webhook", path = "/api/v1/me/webhooks/{webhook}", tag = "webhooks", security(("cookie_session" = [])),
    params(("webhook" = i64, Path), ("X-CSRF-Token" = String, Header)),
    responses((status = 204, description = "Webhook deleted"), (status = 401, description = "Browser authentication required", body = ErrorResponse), (status = 403, description = "CSRF token missing or invalid", body = ErrorResponse), (status = 404, description = "Webhook not found", body = ErrorResponse)))]
pub(crate) async fn remove(
    Path(id): Path<i64>,
    State(state): State<ApiState>,
    BrowserAuthenticatedPlayer(player): BrowserAuthenticatedPlayer,
) -> Result<StatusCode, ApiError> {
    if !crate::models::Webhook::delete_owned(&state.database, id, player.id)
        .await
        .map_err(ApiError::database)?
    {
        return Err(ApiError::not_found("webhook_not_found", "Webhook"));
    }
    Ok(StatusCode::NO_CONTENT)
}
