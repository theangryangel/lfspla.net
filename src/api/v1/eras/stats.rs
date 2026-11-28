//! Cached, public statistics for one era.
use crate::api::{ApiError, ApiState, ErrorResponse, extractors as extract};
use axum::{Json, extract::State};
use axum_response_cache::CacheLayer;
use serde::Serialize;
use std::time::Duration;
use utoipa::ToSchema;
use utoipa_axum::{router::OpenApiRouter, routes};

#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct EraStatsResponse {
    /// All validated laps in this era, including older personal bests and imports.
    validated_hotlaps: i64,
    /// Configured rankings in this era.
    rankings: i64,
    /// Distinct charts offered by at least one ranking, including unclaimed charts.
    charts: i64,
}

pub(super) fn router() -> OpenApiRouter<ApiState> {
    OpenApiRouter::new()
        .routes(routes!(get_era_stats))
        .route_layer(
            // Public, unfiltered totals: share the homepage's one-hour lifetime.
            // Include the era path but ignore query strings and authentication.
            CacheLayer::with_lifespan_and_keyer(
                Duration::from_secs(60 * 60),
                |request: &axum::extract::Request| {
                    (request.method().clone(), request.uri().path().to_owned())
                },
            ),
        )
}

#[utoipa::path(
    get, path = "/api/v1/eras/{era}/stats", operation_id = "get_era_stats", tag = "eras",
    params(("era" = String, Path, description = "Era slug")),
    responses(
        (status = 200, description = "Era statistics, cached for up to one hour", body = EraStatsResponse),
        (status = 404, description = "Era not found", body = ErrorResponse),
        (status = 500, description = "Statistics could not be loaded", body = ErrorResponse)
    )
)]
pub(crate) async fn get_era_stats(
    extract::Era(era): extract::Era,
    State(state): State<ApiState>,
) -> Result<Json<EraStatsResponse>, ApiError> {
    let stats = era
        .stats(&state.database)
        .await
        .map_err(ApiError::database)?;
    Ok(Json(EraStatsResponse {
        validated_hotlaps: stats.validated_hotlaps,
        rankings: stats.rankings,
        charts: stats.charts,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tower::ServiceExt;

    async fn body(app: &axum::Router, path: &str) -> (axum::http::StatusCode, serde_json::Value) {
        let response = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .uri(path)
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        (status, serde_json::from_slice(&bytes).unwrap())
    }

    #[sqlx::test(migrator = "lfsplanet_core::db::MIGRATOR")]
    #[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
    async fn era_stats_count_and_cache_independently(pool: sqlx::PgPool) -> anyhow::Result<()> {
        sqlx::raw_sql(lfsplanet_hotlaps::test_support::FIXTURE_SQL)
            .execute(&pool)
            .await?;
        sqlx::raw_sql(r"
            UPDATE hotlap SET state = 'valid', hlvc_result_code = 1, finished_at = now();
            INSERT INTO era (slug, title, version_requirement, installation_id)
                VALUES ('empty-era', 'Empty', '>=0.7A', '0.7');
            INSERT INTO ranking (era_id, slug, position, title, description, benchmark_percent, nation_max_points, nation_driver_limit)
                SELECT id, 'shared', 1, 'Shared chart', 'A second ranking using the same chart', 103, 10, 3 FROM era WHERE slug = '2026-09-23';
            INSERT INTO ranking_chart_membership (era_id, ranking_id, position, chart_id)
                SELECT ranking.era_id, ranking.id, 0, chart.id FROM ranking
                JOIN chart ON chart.era_id = ranking.era_id WHERE ranking.slug = 'shared';
            INSERT INTO chart (era_id, track_id, vehicle_id)
                SELECT id, 'BL1', 'XFG' FROM era WHERE slug = 'empty-era';
        ").execute(&pool).await?;
        let state = ApiState::for_tests(sea_orm::SqlxPostgresConnector::from_sqlx_postgres_pool(
            pool.clone(),
        ));
        let app =
            crate::api::router(state.hotlaps.max_spr_upload_bytes()).with_state(state.clone());
        let (status, totals) = body(&app, "/api/v1/eras/2026-09-23/stats").await;
        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(
            totals,
            serde_json::json!({"validated_hotlaps": 1, "rankings": 2, "charts": 1})
        );
        assert_eq!(
            body(&app, "/api/v1/eras/empty-era/stats").await.1,
            serde_json::json!({"validated_hotlaps": 0, "rankings": 0, "charts": 0})
        );
        sqlx::query("DELETE FROM hotlap").execute(&pool).await?;
        assert_eq!(
            body(&app, "/api/v1/eras/2026-09-23/stats?irrelevant=1")
                .await
                .1,
            totals
        );
        let fresh = crate::api::router(state.hotlaps.max_spr_upload_bytes()).with_state(state);
        assert_eq!(
            body(&fresh, "/api/v1/eras/2026-09-23/stats").await.1["validated_hotlaps"],
            0
        );
        assert_eq!(
            body(&app, "/api/v1/eras/missing/stats").await.0,
            axum::http::StatusCode::NOT_FOUND
        );
        Ok(())
    }
}
