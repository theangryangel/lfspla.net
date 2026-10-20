//! Public homepage totals and activity spotlights.
use crate::{
    api::v1::{
        PlayerSummary,
        eras::{response::Chart, tracks::TrackSummary, vehicles::VehicleSummary},
    },
    api::{ApiError, ApiState, ErrorResponse},
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
    /// Ordered, eligible homepage cards. Unsupported selections are omitted.
    spotlights: Vec<SpotlightResponse>,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct SpotlightResponse {
    id: String,
    title: String,
    highlight_label: String,
    highlight_value: i64,
    /// Render the numeric highlight as a lap-time gap when true.
    milliseconds: bool,
    /// Render basis points as a percentage (100 = 1%).
    percentage: bool,
    content: SpotlightContent,
}

#[derive(Serialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum SpotlightContent {
    Driver { player: PlayerSummary },
    Combo { chart: Box<Chart> },
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
    let locations = super::track_locations::TrackLocationSummary::load(&state.database).await?;
    let convert_combo = |combo: crate::models::site_stats::ComboSpotlight| -> Result<_, ApiError> {
        Ok(Chart::new(
            combo.era_id,
            combo.era_title,
            TrackSummary::new(
                combo.track,
                combo.track_name,
                super::track_locations::TrackLocationSummary::find(
                    &locations,
                    &combo.track_location,
                )?,
                combo.track_reverse,
                combo.track_open_configuration,
                combo.track_image_public_id.map(|id| {
                    crate::api::v1::attachments::Attachment::new(
                        id,
                        combo.track_image_original_filename,
                    )
                }),
                combo.track_outline_public_id.map(|id| {
                    crate::api::v1::attachments::Attachment::new(
                        id,
                        combo.track_outline_original_filename,
                    )
                }),
            ),
            VehicleSummary::new(
                combo.vehicle,
                combo.vehicle_name,
                combo.vehicle_license,
                combo.vehicle_image_public_id.map(|id| {
                    crate::api::v1::attachments::Attachment::new(
                        id,
                        combo.vehicle_image_original_filename,
                    )
                }),
            ),
        ))
    };
    let mut spotlights = Vec::new();
    for spotlight in totals.spotlights {
        let content = if let Some(combo) = spotlight.combo {
            SpotlightContent::Combo {
                chart: Box::new(convert_combo(combo)?),
            }
        } else if let Some(driver) = spotlight.driver {
            SpotlightContent::Driver {
                player: driver.player.into(),
            }
        } else {
            continue;
        };
        spotlights.push(SpotlightResponse {
            id: spotlight.id.into(),
            title: spotlight.title.into(),
            highlight_label: spotlight.label.into(),
            highlight_value: spotlight.value,
            milliseconds: spotlight.milliseconds,
            percentage: spotlight.percentage,
            content,
        });
    }
    Ok(Json(StatsResponse {
        validated_hotlaps: totals.validated_hotlaps,
        drivers: totals.drivers,
        combinations: totals.combinations,
        eras: totals.eras,
        spotlight_uploads: totals.spotlight_uploads,
        spotlights,
    }))
}
