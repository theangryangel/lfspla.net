//! Vehicle catalogue endpoints scoped to an era.

use axum::{
    Json,
    extract::{Query, State},
};
use sea_orm::QuerySelect;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::{router::OpenApiRouter, routes};
use validator::Validate;

use crate::{
    api::{ApiError, ApiState, ErrorResponse, ListResponse, PER_PAGE, extractors as extract},
    models::vehicles::{VehicleFilter, VehicleModel, VehicleOrder},
};

const MAX_SEARCH_LIMIT: u32 = 100;

/// Metadata for one canonical Live for Speed vehicle.
#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct VehicleSummary {
    code: String,
    name: String,
    license: String,
    /// Relative URL of a locally cached vehicle image, when available.
    image_url: Option<String>,
}

/// Search and pagination for the era vehicle catalogue.
#[derive(Debug, Deserialize, IntoParams, Validate)]
#[into_params(parameter_in = Query)]
pub(crate) struct VehicleSearchQuery {
    /// Restrict suggestions to combinations on this canonical track.
    track: Option<String>,
    /// Vehicle code or name search text.
    q: Option<String>,
    /// Maximum number of vehicles to return (1-100).
    #[validate(range(
        min = 1,
        max = "MAX_SEARCH_LIMIT",
        message = "The page size must be between 1 and 100"
    ))]
    limit: Option<u32>,
}

/// Builds era-scoped vehicle catalogue routes.
pub(super) fn router() -> OpenApiRouter<ApiState> {
    OpenApiRouter::new().routes(routes!(list_for_era))
}

#[utoipa::path(
    get,
    path = "/api/v1/eras/{era}/vehicles",
    operation_id = "list_era_vehicles",
    tag = "vehicles",
    params(
        ("era" = String, Path, description = "Era slug"),
        VehicleSearchQuery,
    ),
    responses(
        (status = 200, description = "Vehicles available in the era", body = ListResponse<VehicleSummary>),
        (status = 400, description = "Invalid page size", body = ErrorResponse),
        (status = 404, description = "Era was not found", body = ErrorResponse)
    )
)]
pub(crate) async fn list_for_era(
    extract::Era(era): extract::Era,
    State(state): State<ApiState>,
    Query(query): Query<VehicleSearchQuery>,
) -> Result<Json<ListResponse<VehicleSummary>>, ApiError> {
    let search = query.q.as_deref().unwrap_or("");
    query.validate()?;
    let limit = query.limit.unwrap_or(PER_PAGE);
    let track = query
        .track
        .as_deref()
        .filter(|track| !track.is_empty())
        .map(|track| {
            track
                .to_ascii_uppercase()
                .parse::<insim_core::track::Track>()
        })
        .transpose()
        .map_err(|_| {
            ApiError::new(
                axum::http::StatusCode::BAD_REQUEST,
                "invalid_track",
                "The track code is invalid",
            )
        })?;
    let vehicles: Vec<_> = era
        .vehicles_for_track(track.as_ref())
        .name_or_id_contains(search)
        .in_catalogue_search_order(search)
        .limit(u64::from(limit))
        .all(&state.database)
        .await
        .map_err(ApiError::database)?
        .into_iter()
        .map(VehicleSummary::from)
        .collect();
    Ok(Json(ListResponse::from(vehicles)))
}

impl From<VehicleModel> for VehicleSummary {
    fn from(vehicle: VehicleModel) -> Self {
        Self {
            image_url: vehicle
                .image_object_key
                .is_some()
                .then(|| crate::api::v1::vehicles::image_url(&vehicle.id)),
            code: vehicle.id,
            name: vehicle.name,
            license: vehicle.license,
        }
    }
}
