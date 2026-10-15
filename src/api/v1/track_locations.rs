//! Public locations and their authored catalogue metadata.

use crate::api::{ApiError, ApiState, ErrorResponse, ListResponse};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::Serialize;
use utoipa::ToSchema;
use utoipa_axum::{router::OpenApiRouter, routes};

/// Stable code and display name for an LFS location.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub(crate) struct TrackLocationSummary {
    pub(crate) code: String,
    pub(crate) name: String,
    #[schema(required)]
    pub(crate) description: Option<String>,
    pub(crate) sequence: i32,
    #[schema(required)]
    pub(crate) image: Option<crate::api::v1::attachments::Attachment>,
}

pub(crate) fn router() -> OpenApiRouter<ApiState> {
    OpenApiRouter::new()
        .routes(routes!(list_track_locations))
        .routes(routes!(get_track_location))
}

#[utoipa::path(
    get,
    path = "/api/v1/track-locations",
    operation_id = "list_track_locations",
    tag = "track-locations",
    responses(
        (status = 200, description = "Track locations in display order", body = ListResponse<TrackLocationSummary>),
        (status = 500, description = "Locations could not be loaded", body = ErrorResponse)
    )
)]
pub(crate) async fn list_track_locations(
    State(state): State<ApiState>,
) -> Result<Json<ListResponse<TrackLocationSummary>>, ApiError> {
    let mut locations = TrackLocationSummary::load(&state.database)
        .await?
        .into_values()
        .collect::<Vec<_>>();
    locations.sort_by(|a, b| {
        a.sequence
            .cmp(&b.sequence)
            .then_with(|| a.code.cmp(&b.code))
    });
    Ok(Json(ListResponse::from(locations)))
}

#[utoipa::path(
    get,
    path = "/api/v1/track-locations/{code}",
    operation_id = "get_track_location",
    tag = "track-locations",
    params(("code" = String, Path, description = "Location code")),
    responses(
        (status = 200, description = "Track location metadata", body = TrackLocationSummary),
        (status = 404, description = "Location not found", body = ErrorResponse),
        (status = 500, description = "Location could not be loaded", body = ErrorResponse)
    )
)]
pub(crate) async fn get_track_location(
    Path(code): Path<String>,
    State(state): State<ApiState>,
) -> Result<Json<TrackLocationSummary>, ApiError> {
    use sea_orm::EntityTrait;
    let location = crate::models::track::location::Entity::find_by_id(code.to_ascii_uppercase())
        .one(&state.database)
        .await
        .map_err(ApiError::database)?
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::NOT_FOUND,
                "track_location_not_found",
                "Track location not found",
            )
        })?;
    let attachments = super::attachments::load(
        &state.database,
        location.image_attachment_id.into_iter().collect::<Vec<_>>(),
    )
    .await?;
    Ok(Json(TrackLocationSummary::from_record(
        location,
        &attachments,
    )))
}

impl TrackLocationSummary {
    pub(crate) fn find(
        locations: &std::collections::HashMap<String, Self>,
        code: &str,
    ) -> Result<Self, ApiError> {
        locations.get(code).cloned().ok_or_else(|| {
            ApiError::database(sea_orm::DbErr::RecordNotFound(format!(
                "track location {code} missing"
            )))
        })
    }

    pub(crate) async fn load(
        database: &impl sea_orm::ConnectionTrait,
    ) -> Result<std::collections::HashMap<String, Self>, ApiError> {
        use sea_orm::EntityTrait;
        let locations: Vec<crate::models::TrackLocation> =
            crate::models::track::location::Entity::find()
                .all(database)
                .await
                .map_err(ApiError::database)?;
        let attachments = crate::api::v1::attachments::load(
            database,
            locations
                .iter()
                .filter_map(|location| location.image_attachment_id)
                .collect::<Vec<_>>(),
        )
        .await?;
        Ok(locations
            .into_iter()
            .map(|location| {
                (
                    location.id.clone(),
                    Self::from_record(location, &attachments),
                )
            })
            .collect())
    }
    fn from_record(
        location: crate::models::TrackLocation,
        attachments: &std::collections::HashMap<i64, super::attachments::Attachment>,
    ) -> Self {
        Self {
            code: location.id,
            name: location.name,
            description: location.description,
            sequence: location.sequence,
            image: location
                .image_attachment_id
                .and_then(|id| attachments.get(&id).cloned()),
        }
    }
}
