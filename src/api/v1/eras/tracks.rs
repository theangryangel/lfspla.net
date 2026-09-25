//! Track metadata endpoints scoped to an era.

use axum::{
    Json,
    extract::{Query, State},
};
use insim_core::vehicle::Vehicle;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::{
    api::{ApiError, ApiState, ErrorResponse, ListResponse, extractors as extract},
    models::{
        hotlaps::HotlapRankable,
        tracks::{TrackLocation, TrackModel, TrackOrder},
    },
};

/// Canonical LFS track configuration metadata.
#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct TrackSummary {
    code: String,
    name: String,
    location: TrackLocationSummary,
    reverse: bool,
    open_configuration: bool,
}

/// Narrowing for the era track catalogue.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct TrackSearchQuery {
    /// Restrict tracks to those paired with this canonical vehicle.
    vehicle: Option<String>,
}

/// Stable code and display name for an LFS location.
#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct TrackLocationSummary {
    code: TrackLocation,
    name: String,
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
    // so rankability is what separates a real code from a typo here.
    let vehicle = query
        .vehicle
        .as_deref()
        .filter(|code| !code.is_empty())
        .map(|code| {
            code.to_ascii_uppercase()
                .parse::<Vehicle>()
                .expect("insim_core vehicle parsing is infallible")
                .ensure_hotlap_rankable()
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
    Ok(Json(ListResponse::from(
        tracks
            .into_iter()
            .map(TrackSummary::from)
            .collect::<Vec<_>>(),
    )))
}

impl From<TrackModel> for TrackSummary {
    fn from(track: TrackModel) -> Self {
        Self {
            code: track.id,
            name: track.name,
            location: track.location.into(),
            reverse: track.reverse,
            open_configuration: track.open_configuration,
        }
    }
}

impl From<TrackLocation> for TrackLocationSummary {
    fn from(location: TrackLocation) -> Self {
        Self {
            code: location,
            name: location.name().to_owned(),
        }
    }
}
