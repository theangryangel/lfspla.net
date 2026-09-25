//! Public homepage totals.
use std::time::Duration;

use axum::{Json, extract::State};
use axum_response_cache::CacheLayer;
use serde::Serialize;
use utoipa::ToSchema;
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::{
    api::{ApiError, ApiState, ErrorResponse},
    models::stats::SiteStats,
};

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
    responses((status = 200, body = StatsResponse), (status = 500, body = ErrorResponse)))]
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
    }))
}
