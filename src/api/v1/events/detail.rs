//! Event pages and classified live/final standings.
use crate::{
    api::{ApiError, ApiState, ErrorResponse, PaginatedResponse, PaginationQuery},
    models::event::{
        self,
        game::{EventKind, GameConfig},
        results::Standing,
    },
};
use axum::{
    Json,
    extract::{Path, Query, State},
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::Serialize;
use time::format_description::well_known::Rfc3339;
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
pub(crate) struct EventDetail {
    pub id: i64,
    pub title: String,
    #[schema(required)]
    pub description: Option<String>,
    pub source: EventKind,
    #[schema(required)]
    pub source_url: Option<String>,
    pub starts_at: String,
    #[schema(required)]
    pub ends_at: Option<String>,
    #[schema(required)]
    pub join_url: Option<String>,
    #[schema(required)]
    pub game: Option<GameConfig>,
}
async fn visible(state: &ApiState, id: i64) -> Result<event::Model, ApiError> {
    event::Entity::find_by_id(id)
        .filter(event::Column::Hidden.eq(false))
        .one(&state.database)
        .await
        .map_err(ApiError::database)?
        .ok_or_else(|| ApiError::not_found("event_not_found", "Event"))
}
#[utoipa::path(get, path = "/api/v1/events/{event_id}", operation_id = "get_event", tag = "events",
    params(("event_id" = i64, Path, description = "Event ID")),
    responses((status = 200, body = EventDetail), (status = 404, body = ErrorResponse)))]
pub(crate) async fn get_event(
    Path(id): Path<i64>,
    State(state): State<ApiState>,
) -> Result<Json<EventDetail>, ApiError> {
    let row = visible(&state, id).await?;
    let game = row
        .game()
        .map_err(|e| ApiError::internal(&e, "game configuration"))?;
    let format = |value: time::OffsetDateTime| {
        value
            .format(&Rfc3339)
            .map_err(|e| ApiError::internal(&e, "event date"))
    };
    Ok(Json(EventDetail {
        id: row.id,
        title: row.title,
        description: row.description,
        source: row.source,
        source_url: row.source_url.or(row.details_url),
        starts_at: format(row.starts_at)?,
        ends_at: row.ends_at.map(format).transpose()?,
        join_url: row.signup_url,
        game,
    }))
}
#[utoipa::path(get, path = "/api/v1/events/{event_id}/results", operation_id = "event_results", tag = "events",
    params(("event_id" = i64, Path, description = "Event ID"), PaginationQuery),
    responses((status = 200, body = PaginatedResponse<Standing>), (status = 400, body = ErrorResponse), (status = 404, body = ErrorResponse)))]
pub(crate) async fn event_results(
    Path(id): Path<i64>,
    Query(page): Query<PaginationQuery>,
    State(state): State<ApiState>,
) -> Result<Json<PaginatedResponse<Standing>>, ApiError> {
    let offset = page.offset()?;
    let row = visible(&state, id).await?;
    let (items, total) = event::results::standings(
        state.database.get_postgres_connection_pool(),
        id,
        row.source,
        i64::try_from(page.per_page).unwrap_or(100),
        i64::try_from(offset).unwrap_or(i64::MAX),
    )
    .await
    .map_err(|e| ApiError::internal(&e, "event results"))?;
    Ok(Json(PaginatedResponse {
        items,
        pagination: page.metadata(u64::try_from(total).unwrap_or_default()),
    }))
}
