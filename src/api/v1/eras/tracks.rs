//! Track metadata endpoints scoped to an era.

use crate::api::v1::track_locations::TrackLocationSummary;
use crate::{
    api::{ApiError, ApiState, ErrorResponse, ListResponse, extractors as extract},
    models::{Track as TrackRecord, track::TrackOrder},
};
use axum::{
    Json,
    extract::{Query, State},
};
use insim_core::vehicle::Vehicle;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::{router::OpenApiRouter, routes};

/// Canonical LFS track configuration metadata.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub(crate) struct TrackSummary {
    code: String,
    name: String,
    location: TrackLocationSummary,
    reverse: bool,
    open_configuration: bool,
    #[schema(required)]
    image: Option<crate::api::v1::attachments::Attachment>,
    #[schema(required)]
    outline: Option<crate::api::v1::attachments::Attachment>,
}

/// Narrowing for the era track catalogue.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct TrackSearchQuery {
    /// Restrict tracks to those paired with this canonical vehicle.
    vehicle: Option<String>,
}

/// Builds era-scoped track routes.
pub(super) fn router() -> OpenApiRouter<ApiState> {
    OpenApiRouter::new().routes(routes!(list_for_era))
}

#[utoipa::path(
    get,
    path = "/api/v1/eras/{era}/tracks",
    operation_id = "list_era_tracks",
    tag = "tracks",
    params(
        ("era" = String, Path, description = "Era slug"),
        TrackSearchQuery,
    ),
    responses(
        (status = 200, description = "Tracks available in the era", body = ListResponse<TrackSummary>),
        (status = 400, description = "Invalid vehicle code", body = ErrorResponse),
        (status = 404, description = "Era was not found", body = ErrorResponse)
    )
)]
pub(crate) async fn list_for_era(
    extract::Era(era): extract::Era,
    State(state): State<ApiState>,
    Query(query): Query<TrackSearchQuery>,
) -> Result<Json<ListResponse<TrackSummary>>, ApiError> {
    // Vehicle parsing is infallible - anything unrecognised becomes `Unknown` -
    // reject that sentinel before filtering the catalogue.
    let vehicle = query
        .vehicle
        .as_deref()
        .filter(|code| !code.is_empty())
        .map(|code| {
            let vehicle = code
                .to_ascii_uppercase()
                .parse::<Vehicle>()
                .expect("insim_core vehicle parsing is infallible");
            if vehicle == Vehicle::Unknown {
                Err(())
            } else {
                Ok(vehicle)
            }
        })
        .transpose()
        .map_err(|_| {
            ApiError::new(
                axum::http::StatusCode::BAD_REQUEST,
                "invalid_vehicle",
                "The vehicle code is invalid",
            )
        })?;
    let tracks = era
        .tracks_for_vehicle(vehicle.as_ref())
        .in_catalogue_order()
        .all(&state.database)
        .await
        .map_err(ApiError::database)?;
    let attachments = crate::api::v1::attachments::load(
        &state.database,
        tracks
            .iter()
            .flat_map(|track| [track.image_attachment_id, track.outline_attachment_id])
            .flatten()
            .collect::<Vec<_>>(),
    )
    .await?;
    let locations = TrackLocationSummary::load(&state.database).await?;
    Ok(Json(ListResponse::from(
        tracks
            .into_iter()
            .map(|track| TrackSummary::from_record(track, &attachments, &locations))
            .collect::<Result<Vec<_>, _>>()?,
    )))
}

impl TrackSummary {
    pub(crate) fn new(
        code: crate::track_id::TrackId,
        name: String,
        location: TrackLocationSummary,
        reverse: bool,
        open_configuration: bool,
        image: Option<crate::api::v1::attachments::Attachment>,
        outline: Option<crate::api::v1::attachments::Attachment>,
    ) -> Self {
        Self {
            code: code.to_string(),
            name,
            location,
            reverse,
            open_configuration,
            image,
            outline,
        }
    }
}

impl TrackSummary {
    pub(crate) fn from_record(
        track: TrackRecord,
        attachments: &std::collections::HashMap<i64, crate::api::v1::attachments::Attachment>,
        locations: &std::collections::HashMap<String, TrackLocationSummary>,
    ) -> Result<Self, ApiError> {
        Ok(Self::new(
            track.id,
            track.name,
            TrackLocationSummary::find(locations, &track.location)?,
            track.reverse,
            track.open_configuration,
            track
                .image_attachment_id
                .and_then(|id| attachments.get(&id).cloned()),
            track
                .outline_attachment_id
                .and_then(|id| attachments.get(&id).cloned()),
        ))
    }
}
