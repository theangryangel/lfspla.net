//! Page-paginated best-hotlap leaderboard for one era-scoped chart.

use super::response::Chart;

use crate::{
    api::{
        ApiError, ApiState, ErrorResponse, Ordering, PaginatedResponse, PaginationQuery,
        extractors as extract,
        v1::hotlaps::{
            query::{controller_filter, country_filter},
            response::{Hotlap, RankingContribution, list_response},
        },
    },
    models::{
        Hotlap as HotlapModel,
        hotlap::{HotlapListColumn, HotlapListFilters, HotlapListPage},
    },
};
use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::{router::OpenApiRouter, routes};

/// Builds era-scoped chart routes.
pub(super) fn router() -> OpenApiRouter<ApiState> {
    OpenApiRouter::new().routes(routes!(best))
}

/// Filters and pagination for the chart leaderboard.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct ChartHotlapQuery {
    /// Column to sort by; defaults to rank. Driver uses case-insensitive display name.
    #[serde(default = "rank_column")]
    #[param(inline)]
    column: HotlapListColumn,
    /// Sort direction; defaults to asc. Equal values retain chart rank order.
    #[serde(default)]
    #[param(inline)]
    order: Ordering,
    /// ISO 3166-1 alpha-2 player country.
    country: Option<String>,
    /// Controller: wheel, mouse, keyboard, or keyboard_stabilised.
    controller: Option<String>,
}

fn rank_column() -> HotlapListColumn {
    HotlapListColumn::Rank
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
    chart: Chart,
    #[serde(flatten)]
    page: PaginatedResponse<Hotlap>,
    /// Rankings that include this combination
    contributes_to: Vec<RankingContribution>,
}

#[utoipa::path(
    get,
    path = "/api/v1/eras/{era}/charts/{track}/{vehicle}",
    tag = "charts",
    params(
        ("era" = String, Path, description = "Target era identifier"),
        ("track" = String, Path, description = "Canonical track configuration code"),
        ("vehicle" = String, Path, description = "Canonical vehicle code"),
        ChartHotlapQuery,
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
    Query(query): Query<ChartHotlapQuery>,
    Query(pagination): Query<PaginationQuery>,
    State(state): State<ApiState>,
) -> Result<Json<HotlapChartResponse>, ApiError> {
    let era = extract::resolve_era(&state.database, &path.era).await?;
    let chart = crate::models::Chart::find_combination(
        &state.database,
        era.id,
        &path.track.to_ascii_uppercase(),
        &path.vehicle.to_ascii_uppercase(),
    )
    .await
    .map_err(ApiError::database)?
    .ok_or_else(chart_not_found)?;
    let chart_response = Chart::load(&state.database, &era, std::slice::from_ref(&chart))
        .await?
        .pop()
        .ok_or_else(|| {
            ApiError::database(sea_orm::DbErr::RecordNotFound(
                "hydrated chart missing".into(),
            ))
        })?;
    let mut filters = HotlapListFilters::for_chart(&chart);
    filters.country = country_filter(query.country.as_deref())?;
    filters.controller = controller_filter(query.controller.as_deref())?;
    let page = HotlapModel::list(
        &state.database,
        filters,
        HotlapListPage {
            column: query.column,
            order: query.order,
            offset: pagination.offset()?,
            limit: pagination.per_page,
        },
    )
    .await
    .map_err(ApiError::from)?;
    let contributes_to = page
        .chart_contributions
        .as_ref()
        .map(|contributions| contributions.iter().cloned().map(Into::into).collect())
        .unwrap_or_default();
    Ok(Json(HotlapChartResponse {
        chart: chart_response,
        contributes_to,
        page: list_response(page, &pagination, None),
    }))
}

fn chart_not_found() -> ApiError {
    ApiError::not_found("chart_not_found", "Chart")
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;

    #[test]
    fn empty_leaderboards_still_expose_the_chart_resource() {
        use crate::api::v1::eras::{tracks::TrackSummary, vehicles::VehicleSummary};
        let response = HotlapChartResponse {
            chart: Chart::new(
                "2007-12-21".parse().unwrap(),
                "Historical".to_owned(),
                TrackSummary::new(
                    crate::track_id::TrackId(insim_core::track::Track::Bl1),
                    "Blackwood GP".to_owned(),
                    crate::models::track::TrackLocation::Bl,
                    false,
                    false,
                ),
                VehicleSummary::new(
                    "XFG".parse().unwrap(),
                    "XF GTI".to_owned(),
                    "demo".to_owned(),
                    false,
                ),
            ),
            page: PaginatedResponse {
                items: vec![],
                pagination: PaginationQuery::default().metadata(0),
            },
            contributes_to: vec![],
        };
        let json = serde_json::to_value(response).unwrap();
        assert_eq!(json["chart"]["era_id"], "2007-12-21");
        assert_eq!(json["chart"]["track"]["code"], "BL1");
        assert_eq!(json["chart"]["vehicle"]["code"], "XFG");
        assert_eq!(
            json["chart"]["vehicle"]["image_url"],
            serde_json::Value::Null
        );
        assert_eq!(json["items"], serde_json::json!([]));
        assert_eq!(json["pagination"]["total_items"], 0);
        for old in ["era_id", "track", "vehicle"] {
            assert!(json.get(old).is_none());
        }
    }

    #[test]
    fn chart_filters_and_shared_pagination_deserialize_together() {
        let search = "country=GB&controller=wheel&page=3&per_page=25";
        let query: ChartHotlapQuery = serde_urlencoded::from_str(search).unwrap();
        let pagination: PaginationQuery = serde_urlencoded::from_str(search).unwrap();
        assert_eq!(query.country.as_deref(), Some("GB"));
        assert_eq!(query.controller.as_deref(), Some("wheel"));
        assert_eq!(pagination.offset().unwrap(), 50);
        let pagination: PaginationQuery = serde_urlencoded::from_str("country=GB").unwrap();
        assert_eq!(pagination.offset().unwrap(), 0);
    }
    #[test]
    fn chart_sort_values_are_typed_and_default_to_rank_ascending() {
        let defaults: ChartHotlapQuery = serde_urlencoded::from_str("").unwrap();
        assert_eq!(defaults.column, HotlapListColumn::Rank);
        assert_eq!(defaults.order, Ordering::Asc);
        let legacy: ChartHotlapQuery = serde_urlencoded::from_str("column=set").unwrap();
        assert_eq!(legacy.column, HotlapListColumn::Submitted);
        for column in [
            HotlapListColumn::Rank,
            HotlapListColumn::Driver,
            HotlapListColumn::Submitted,
            HotlapListColumn::LapTime,
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
                let query: ChartHotlapQuery = serde_urlencoded::from_str(&encoded).unwrap();
                assert_eq!(query.column, column);
                assert_eq!(query.order, order);
            }
        }
    }

    #[tokio::test]
    async fn page_two_is_accepted_by_http_query_extractors() {
        use axum::Router;
        use axum::body::Body;
        use axum::http::Request;
        use axum::routing::get;
        use tower::ServiceExt;

        async fn extract_page(
            Query(_filters): Query<ChartHotlapQuery>,
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
