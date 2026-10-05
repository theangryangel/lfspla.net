//! Public homepage totals and activity spotlights.
use crate::{
    api::v1::PlayerSummary,
    api::{ApiError, ApiState, ErrorResponse},
    era_slug::EraSlug,
    milliseconds::Milliseconds,
    models::site_stats::SiteStats,
};
use axum::{Json, extract::State};
use axum_response_cache::CacheLayer;
use serde::Serialize;
use std::time::Duration;
use utoipa::ToSchema;
use utoipa_axum::{router::OpenApiRouter, routes};
#[derive(Serialize, ToSchema)]
pub(crate) struct StatsResponse {
    /// All hotlaps whose validation state is valid.
    validated_hotlaps: i64,
    /// All driver accounts, including imported drivers.
    drivers: i64,
    /// Distinct track/vehicle pairs selected by rankings, deduplicated across eras.
    combinations: i64,
    /// All eras, including closed historical eras.
    eras: i64,
    /// Number of validated uploads sampled, capped at 100. Includes demo laps; excludes historical imports.
    spotlight_uploads: i64,
    /// Busiest chart in the sample, with its current top three drivers. Null when there are no uploads.
    #[schema(required)]
    combo_spotlight: Option<ComboSpotlightResponse>,
    /// Driver with the most current personal bests in the sample. Null when none remain current PBs.
    #[schema(required)]
    driver_spotlight: Option<DriverSpotlightResponse>,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct ComboSpotlightResponse {
    era_id: EraSlug,
    era_title: String,
    track: String,
    track_name: String,
    vehicle: String,
    vehicle_name: String,
    #[schema(required)]
    vehicle_image_url: Option<String>,
    recent_uploads: i64,
    leaders: Vec<SpotlightBestResponse>,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct SpotlightBestResponse {
    player: PlayerSummary,
    position: i64,
    lap_time_ms: Milliseconds,
    distance_to_world_record_ms: Milliseconds,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct DriverSpotlightResponse {
    player: PlayerSummary,
    recent_personal_bests: i64,
}

pub(super) fn router() -> OpenApiRouter<ApiState> {
    OpenApiRouter::new().routes(routes!(get_stats)).route_layer(
        // This endpoint has no parameters or personalised responses. Ignore
        // query strings so arbitrary URLs cannot grow the cache.
        CacheLayer::with_lifespan_and_keyer(
            Duration::from_secs(60 * 60),
            |request: &axum::extract::Request| request.method().clone(),
        ),
    )
}

#[utoipa::path(get, path = "/api/v1/stats", operation_id = "get_stats", tag = "stats",
    responses((status = 200, description = "Site statistics", body = StatsResponse), (status = 500, description = "Statistics could not be loaded", body = ErrorResponse)))]
pub(crate) async fn get_stats(
    State(state): State<ApiState>,
) -> Result<Json<StatsResponse>, ApiError> {
    let totals = SiteStats::load(&state.database)
        .await
        .map_err(ApiError::database)?;
    Ok(Json(StatsResponse {
        validated_hotlaps: totals.validated_hotlaps,
        drivers: totals.drivers,
        combinations: totals.combinations,
        eras: totals.eras,
        spotlight_uploads: totals.spotlight_uploads,
        combo_spotlight: totals.combo_spotlight.map(|combo| ComboSpotlightResponse {
            era_id: combo.era_id,
            era_title: combo.era_title,
            track: combo.track.to_string(),
            track_name: combo.track_name,
            vehicle: combo.vehicle.to_string(),
            vehicle_name: combo.vehicle_name,
            vehicle_image_url: combo
                .vehicle_has_image
                .then(|| crate::api::v1::vehicles::image_url(&combo.vehicle.to_string())),
            recent_uploads: combo.recent_uploads,
            leaders: combo
                .leaders
                .into_iter()
                .map(|entry| SpotlightBestResponse {
                    player: entry.player.into(),
                    position: entry.position,
                    lap_time_ms: entry.lap_time_ms,
                    distance_to_world_record_ms: entry.distance_to_world_record_ms,
                })
                .collect(),
        }),
        driver_spotlight: totals
            .driver_spotlight
            .map(|driver| DriverSpotlightResponse {
                player: driver.player.into(),
                recent_personal_bests: driver.recent_personal_bests,
            }),
    }))
}
