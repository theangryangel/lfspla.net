//! Page-paginated best-hotlap leaderboard for one era-scoped chart.

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use celes::Country;
use insim_core::{track::Track, vehicle::Vehicle};
use sea_orm::ActiveEnum;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::{
    api::{
        ApiError, ApiState, ErrorResponse, Ordering, PaginatedResponse, PaginationQuery,
        extractors as extract, v1::PlayerSummary,
    },
    models::{
        badges::PlayerBadge,
        hotlaps::{
            self, BestHotlap, BestHotlapColumn, BestHotlapFilters, BestHotlapPage, DriverSide,
            HotlapRankable, SteeringInput,
        },
    },
};

/// Builds era-scoped chart routes.
pub(super) fn router() -> OpenApiRouter<ApiState> {
    OpenApiRouter::new().routes(routes!(best))
}

/// Filters and pagination for the chart leaderboard.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct BestHotlapQuery {
    /// Column to sort by; defaults to rank. Driver uses case-insensitive display name.
    #[serde(default)]
    #[param(inline)]
    column: BestHotlapColumn,
    /// Sort direction; defaults to asc. Equal values retain chart rank order.
    #[serde(default)]
    #[param(inline)]
    order: Ordering,
    /// ISO 3166-1 alpha-2 player country.
    country: Option<String>,
    /// Controller: wheel, mouse, keyboard, or keyboard_stabilised.
    controller: Option<String>,
}

/// A player profile attached to a public leaderboard entry.
#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct BestHotlapPlayerResponse {
    #[serde(flatten)]
    player: PlayerSummary,
    badges: Vec<PlayerBadge>,
}

/// One player's fastest valid lap for the requested chart and era.
#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct BestHotlapResponse {
    /// Global chart position, unchanged by filters or pagination.
    position: i64,
    id: i64,
    /// Relative URL of the original replay, absent when none is stored.
    #[schema(required)]
    spr_url: Option<String>,
    player: BestHotlapPlayerResponse,
    track: String,
    vehicle: String,
    lap_time_ms: i64,
    distance_to_benchmark_ms: i64,
    distance_to_world_record_ms: i64,
    split_1_ms: i64,
    split_2_ms: i64,
    split_3_ms: i64,
    split_4_ms: i64,
    steering: SteeringInput,
    brake_help_enabled: bool,
    automatic_gears: bool,
    #[schema(required)]
    manual_shifter: Option<bool>,
    axis_clutch: bool,
    automatic_clutch: bool,
    driver_side: DriverSide,
    #[schema(required)]
    abs_enabled: Option<bool>,
    #[serde(with = "time::serde::rfc3339")]
    #[schema(value_type = String, format = DateTime)]
    created_at: time::OffsetDateTime,
    game_version: String,
}

/// Composite identity of one hotlap chart.
#[derive(Debug, Deserialize)]
pub(crate) struct HotlapChartPath {
    era: String,
    track: String,
    vehicle: String,
}

/// Page-paginated representation of one hotlap chart.
#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct HotlapChartResponse {
    era_id: String,
    track: String,
    vehicle: String,
    #[serde(flatten)]
    page: PaginatedResponse<BestHotlapResponse>,
}

#[utoipa::path(
    get,
    path = "/api/v1/eras/{era}/charts/{track}/{vehicle}",
    tag = "charts",
    params(
        ("era" = String, Path, description = "Target era identifier"),
        ("track" = String, Path, description = "Canonical track configuration code"),
        ("vehicle" = String, Path, description = "Canonical vehicle code"),
        BestHotlapQuery,
        PaginationQuery,
    ),
    responses(
        (status = 200, description = "Hotlap chart with the fastest valid lap per player", body = HotlapChartResponse),
        (status = 400, description = "Invalid optional filter or pagination value", body = ErrorResponse),
        (status = 404, description = "Era or chart not found", body = ErrorResponse)
    )
)]
pub(crate) async fn best(
    Path(path): Path<HotlapChartPath>,
    Query(query): Query<BestHotlapQuery>,
    Query(pagination): Query<PaginationQuery>,
    State(state): State<ApiState>,
) -> Result<Json<HotlapChartResponse>, ApiError> {
    let era = extract::resolve_era(&state.database, &path.era).await?;
    let track: Track = path.track.parse().map_err(|_| chart_not_found())?;
    let vehicle = path
        .vehicle
        .parse::<Vehicle>()
        .expect("insim_core vehicle parsing is infallible")
        .ensure_hotlap_rankable()
        .map_err(|_| chart_not_found())?;
    if !era
        .admits_combination(&state.database, &track, &vehicle)
        .await
        .map_err(ApiError::database)?
    {
        return Err(chart_not_found());
    }
    let track_id = track.to_string();
    let vehicle_id = vehicle.to_string();
    let country = query
        .country
        .as_deref()
        .map(Country::from_alpha2)
        .transpose()
        .map_err(|_| invalid_country_filter())?;
    let controller = query
        .controller
        .as_ref()
        .map(SteeringInput::try_from_value)
        .transpose()
        .map_err(|_| invalid_controller_filter())?;
    let offset = pagination.offset()?;

    let (entries, total) = hotlaps::list_best(
        &state.database,
        BestHotlapFilters {
            era_id: era.id,
            track,
            vehicle,
            country,
            controller,
        },
        BestHotlapPage {
            offset,
            limit: pagination.per_page,
            column: query.column,
            order: query.order,
        },
    )
    .await
    .map_err(ApiError::database)?;
    let mut badges_by_player = era
        .list_badges_for_players(&state.database, entries.iter().map(|entry| entry.player.id))
        .await
        .map_err(ApiError::database)?;

    Ok(Json(HotlapChartResponse {
        era_id: era.slug,
        track: track_id,
        vehicle: vehicle_id,
        page: PaginatedResponse {
            items: entries
                .into_iter()
                .map(|entry| {
                    let badges = badges_by_player
                        .remove(&entry.player.id)
                        .unwrap_or_default();
                    BestHotlapResponse::from_best(entry, badges)
                })
                .collect::<Result<Vec<_>, _>>()
                .map_err(ApiError::database)?,
            pagination: pagination.metadata(total),
        },
    }))
}

fn chart_not_found() -> ApiError {
    ApiError::not_found("chart_not_found", "Chart")
}

fn invalid_country_filter() -> ApiError {
    ApiError::new(
        StatusCode::BAD_REQUEST,
        "invalid_country_filter",
        "The country filter must be an ISO 3166-1 alpha-2 code",
    )
}

fn invalid_controller_filter() -> ApiError {
    ApiError::new(
        StatusCode::BAD_REQUEST,
        "invalid_controller_filter",
        "The controller filter is invalid",
    )
}

impl BestHotlapResponse {
    pub(crate) fn from_best(
        best: BestHotlap,
        badges: Vec<PlayerBadge>,
    ) -> Result<Self, sea_orm::DbErr> {
        let controls = best.hotlap.controls();
        let vehicle = best.hotlap.vehicle.as_ref().ok_or_else(|| {
            sea_orm::DbErr::Type(format!("valid hotlap {} has no vehicle", best.hotlap.id))
        })?;
        Ok(Self {
            position: best.position,
            id: best.hotlap.id,
            spr_url: best
                .hotlap
                .spr_object_key
                .as_ref()
                .map(|_| crate::api::v1::hotlaps::replay::download_url(best.hotlap.id)),
            player: BestHotlapPlayerResponse {
                player: best.player.into(),
                badges,
            },
            track: best.hotlap.track.to_string(),
            vehicle: vehicle.to_string(),
            lap_time_ms: best.hotlap.lap_time_ms,
            distance_to_benchmark_ms: best.distance_to_benchmark_ms,
            distance_to_world_record_ms: best.distance_to_world_record_ms,
            split_1_ms: best.hotlap.split_1_ms,
            split_2_ms: best.hotlap.split_2_ms,
            split_3_ms: best.hotlap.split_3_ms,
            split_4_ms: best.hotlap.split_4_ms,
            steering: controls.steering,
            brake_help_enabled: controls.brake_help_enabled,
            automatic_gears: controls.automatic_gears,
            manual_shifter: controls.manual_shifter,
            axis_clutch: controls.axis_clutch,
            automatic_clutch: controls.automatic_clutch,
            driver_side: controls.driver_side,
            abs_enabled: best.hotlap.abs_enabled,
            created_at: best.hotlap.created_at,
            game_version: best.hotlap.game_version.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chart_filters_and_shared_pagination_deserialize_together() {
        let search = "country=GB&controller=wheel&page=3&per_page=25";
        let query: BestHotlapQuery = serde_urlencoded::from_str(search).unwrap();
        let pagination: PaginationQuery = serde_urlencoded::from_str(search).unwrap();
        assert_eq!(query.country.as_deref(), Some("GB"));
        assert_eq!(query.controller.as_deref(), Some("wheel"));
        assert_eq!(pagination.offset().unwrap(), 50);
        let pagination: PaginationQuery = serde_urlencoded::from_str("country=GB").unwrap();
        assert_eq!(pagination.offset().unwrap(), 0);
    }
    #[test]
    fn chart_sort_values_are_typed_and_default_to_rank_ascending() {
        let defaults: BestHotlapQuery = serde_urlencoded::from_str("").unwrap();
        assert_eq!(defaults.column, BestHotlapColumn::Rank);
        assert_eq!(defaults.order, Ordering::Asc);
        for column in [
            BestHotlapColumn::Rank,
            BestHotlapColumn::Driver,
            BestHotlapColumn::Set,
        ] {
            for order in [Ordering::Asc, Ordering::Desc] {
                let encoded = serde_urlencoded::to_string([
                    (
                        "column",
                        serde_json::to_value(column).unwrap().as_str().unwrap(),
                    ),
                    (
                        "order",
                        serde_json::to_value(order).unwrap().as_str().unwrap(),
                    ),
                ])
                .unwrap();
                let query: BestHotlapQuery = serde_urlencoded::from_str(&encoded).unwrap();
                assert_eq!(query.column, column);
                assert_eq!(query.order, order);
            }
        }
    }

    #[tokio::test]
    async fn page_two_is_accepted_by_http_query_extractors() {
        use axum::{Router, body::Body, http::Request, routing::get};
        use tower::ServiceExt;

        async fn extract_page(
            Query(_filters): Query<BestHotlapQuery>,
            Query(pagination): Query<PaginationQuery>,
        ) -> Result<Json<u64>, ApiError> {
            Ok(Json(pagination.offset()?))
        }

        let router = Router::new().route("/chart", get(extract_page));
        for (query, status) in [
            ("page=2", StatusCode::OK),
            (
                "page=2&per_page=25&country=GB&controller=wheel",
                StatusCode::OK,
            ),
            ("page=2&column=driver&order=desc", StatusCode::OK),
            ("column=unknown", StatusCode::BAD_REQUEST),
            ("order=unknown", StatusCode::BAD_REQUEST),
            ("page=0", StatusCode::BAD_REQUEST),
            ("per_page=0", StatusCode::BAD_REQUEST),
            ("per_page=101", StatusCode::BAD_REQUEST),
            ("page=nope", StatusCode::BAD_REQUEST),
        ] {
            let response = router
                .clone()
                .oneshot(
                    Request::builder()
                        .uri(format!("/chart?{query}"))
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), status, "{query}");
            if status == StatusCode::OK {
                let body = axum::body::to_bytes(response.into_body(), 100)
                    .await
                    .unwrap();
                let offset: u64 = serde_json::from_slice(&body).unwrap();
                assert_eq!(offset, if query.contains("per_page") { 25 } else { 50 });
            }
        }
    }
}
