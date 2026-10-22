//! Public event advertisements, ordered by meeting start time.

use crate::{
    api::{ApiError, ApiState, ErrorResponse, PaginatedResponse, PaginationQuery},
    models::event::{EventListFilters, Model as Event},
};
use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
};
use sea_orm::{AccessMode, IsolationLevel, TransactionTrait};
use serde::{Deserialize, Serialize};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::{router::OpenApiRouter, routes};

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct EventQuery {
    /// Local league ID.
    league_id: Option<i64>,
    /// Local season ID.
    season_id: Option<i64>,
    /// Inclusive RFC 3339 start; defaults to the current time.
    starts_from: Option<String>,
    /// Exclusive RFC 3339 end.
    starts_before: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct EventLeague {
    id: i64,
    name: String,
    #[schema(required)]
    image: Option<super::attachments::Attachment>,
}

#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct EventSeason {
    id: i64,
    name: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct EventSummary {
    id: i64,
    league: EventLeague,
    #[schema(required)]
    season: Option<EventSeason>,
    title: String,
    #[schema(required)]
    description: Option<String>,
    #[schema(format = DateTime)]
    starts_at: String,
    #[schema(required)]
    details_url: Option<String>,
    #[schema(required)]
    signup_url: Option<String>,
    #[schema(required)]
    track_label: Option<String>,
    #[schema(required)]
    cars_label: Option<String>,
}

impl EventQuery {
    fn filter(self, now: OffsetDateTime) -> Result<EventListFilters, ApiError> {
        let invalid = || {
            ApiError::new(
                StatusCode::BAD_REQUEST,
                "invalid_event_filter",
                "Use positive league/season IDs and RFC 3339 dates with starts_before later than starts_from",
            )
        };
        let from = self
            .starts_from
            .as_deref()
            .map(|value| OffsetDateTime::parse(value, &Rfc3339))
            .transpose()
            .map_err(|_| invalid())?
            .unwrap_or(now);
        let before = self
            .starts_before
            .as_deref()
            .map(|value| OffsetDateTime::parse(value, &Rfc3339))
            .transpose()
            .map_err(|_| invalid())?;
        if self.league_id.is_some_and(|id| id <= 0)
            || self.season_id.is_some_and(|id| id <= 0)
            || before.is_some_and(|before| before <= from)
        {
            return Err(invalid());
        }
        Ok(EventListFilters {
            starts_from: from,
            starts_before: before,
            league_id: self.league_id,
            season_id: self.season_id,
        })
    }
}

pub(crate) fn router() -> OpenApiRouter<ApiState> {
    OpenApiRouter::new().routes(routes!(list_events))
}

#[utoipa::path(get, path = "/api/v1/events", operation_id = "list_events", tag = "events",
    params(EventQuery, PaginationQuery),
    responses((status = 200, description = "Visible event advertisements ordered by start time", body = PaginatedResponse<EventSummary>),
        (status = 400, description = "Invalid event filter or pagination", body = ErrorResponse),
        (status = 500, description = "Events could not be loaded", body = ErrorResponse)))]
pub(crate) async fn list_events(
    Query(query): Query<EventQuery>,
    Query(pagination): Query<PaginationQuery>,
    State(state): State<ApiState>,
) -> Result<Json<PaginatedResponse<EventSummary>>, ApiError> {
    let offset = pagination.offset()?;
    let filter = query.filter(OffsetDateTime::now_utc())?;
    let transaction = state
        .database
        .begin_with_config(
            Some(IsolationLevel::RepeatableRead),
            Some(AccessMode::ReadOnly),
        )
        .await
        .map_err(ApiError::database)?;
    let (items, total) = Event::list(&transaction, &filter, offset, pagination.per_page)
        .await
        .map_err(ApiError::database)?;
    let images = super::attachments::load(
        &transaction,
        items
            .iter()
            .filter_map(|row| row.league_image_attachment_id),
    )
    .await?;
    transaction.commit().await.map_err(ApiError::database)?;
    let items = items
        .into_iter()
        .map(|row| {
            Ok(EventSummary {
                id: row.id,
                league: EventLeague {
                    id: row.league_id,
                    name: row.league_name,
                    image: row
                        .league_image_attachment_id
                        .and_then(|id| images.get(&id).cloned()),
                },
                season: row
                    .season_id
                    .zip(row.season_name)
                    .map(|(id, name)| EventSeason { id, name }),
                title: row.title,
                description: row.description,
                starts_at: row
                    .starts_at
                    .format(&Rfc3339)
                    .map_err(|error| ApiError::internal(&error, "event date"))?,
                details_url: row.details_url,
                signup_url: row.signup_url,
                track_label: row.track_label,
                cars_label: row.cars_label,
            })
        })
        .collect::<Result<Vec<_>, ApiError>>()?;
    Ok(Json(PaginatedResponse {
        items,
        pagination: pagination.metadata(total),
    }))
}

#[cfg(test)]
mod tests;
