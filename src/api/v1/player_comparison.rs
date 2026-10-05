//! Comparison of two players' current personal bests.

use crate::{
    api::{
        ApiError, ApiState, ErrorResponse, extractors as extract,
        v1::{PlayerSummary, players::response},
    },
    models::player::{Entity as PlayerEntity, PlayerComparison, PlayerFilter},
};
use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
};
use insim_core::{track::Track, vehicle::Vehicle};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::{router::OpenApiRouter, routes};
use validator::{Validate, ValidationError};
pub(super) fn router() -> OpenApiRouter<ApiState> {
    OpenApiRouter::new().routes(routes!(compare))
}

/// The two drivers and the optional chart filter to compare them on.
#[derive(Debug, Deserialize, IntoParams, Validate)]
#[validate(schema(
    function = "validate_distinct_players",
    skip_on_field_errors = false,
    message = "Choose two different drivers"
))]
#[into_params(parameter_in = Query)]
pub(crate) struct PlayerComparisonQuery {
    /// Stable username for the first driver.
    left: String,
    /// Stable username for the second driver.
    right: String,
    /// Era identifier.
    #[validate(custom(
        function = "crate::validate::validate_non_blank",
        message = "Choose an era"
    ))]
    era: String,
    /// Canonical track configuration code.
    track: Option<String>,
    /// Canonical vehicle code.
    vehicle: Option<String>,
}

/// One side of a current personal-best comparison.
#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct ComparedPlayerResponse {
    player: PlayerSummary,
    results: Vec<response::PlayerChartResultResponse>,
}

/// Current chart results for two LFS accounts.
#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct PlayerComparisonResponse {
    left: ComparedPlayerResponse,
    right: ComparedPlayerResponse,
}

#[utoipa::path(
    get,
    path = "/api/v1/compare",
    tag = "players",
    params(PlayerComparisonQuery),
    responses(
        (status = 200, description = "Current personal-best results for both drivers", body = PlayerComparisonResponse),
        (status = 400, description = "The era is required or the same driver was supplied twice", body = ErrorResponse),
        (status = 404, description = "A driver was not found", body = ErrorResponse)
    )
)]
pub(crate) async fn compare(
    Query(query): Query<PlayerComparisonQuery>,
    State(state): State<ApiState>,
) -> Result<Json<PlayerComparisonResponse>, ApiError> {
    query.validate()?;

    let track = query
        .track
        .filter(|value| !value.trim().is_empty())
        .map(|value| value.to_ascii_uppercase().parse::<Track>())
        .transpose()
        .map_err(|_| invalid_comparison_filter())?;
    let vehicle = query
        .vehicle
        .filter(|value| !value.trim().is_empty())
        .map(|value| {
            let vehicle = value
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
        .map_err(|_| invalid_comparison_filter())?;
    let era_id = query.era.trim();
    let era = extract::resolve_era(&state.database, era_id).await?;
    let track = track.map(|track| track.to_string());
    let vehicle = vehicle.map(|vehicle| vehicle.to_string());
    if track.is_some() || vehicle.is_some() {
        use crate::models::{chart, vehicle as vehicle_model};
        let mut charts = chart::Entity::find().filter(chart::Column::EraId.eq(era.id));
        if let Some(track) = &track {
            charts = charts.filter(chart::Column::TrackId.eq(track));
        }
        if let Some(vehicle) = &vehicle {
            charts = charts
                .filter(chart::Column::VehicleId.eq(vehicle))
                .inner_join(vehicle_model::Entity)
                .filter(vehicle_model::Column::Available.eq(true));
        }
        if charts
            .one(&state.database)
            .await
            .map_err(ApiError::database)?
            .is_none()
        {
            return Err(invalid_comparison_filter());
        }
    }
    let (left, right) = tokio::try_join!(
        PlayerEntity::find()
            .with_username(&query.left)
            .one(&state.database),
        PlayerEntity::find()
            .with_username(&query.right)
            .one(&state.database),
    )
    .map_err(ApiError::database)?;
    let left = left.ok_or_else(|| ApiError::not_found("player_not_found", "Player"))?;
    let right = right.ok_or_else(|| ApiError::not_found("player_not_found", "Player"))?;

    let comparison = left
        .compare_with(
            &state.database,
            right,
            era.id,
            track.as_deref(),
            vehicle.as_deref(),
        )
        .await
        .map_err(ApiError::database)?;

    Ok(Json(PlayerComparisonResponse::from(comparison)))
}

impl From<PlayerComparison> for PlayerComparisonResponse {
    fn from(comparison: PlayerComparison) -> Self {
        Self {
            left: ComparedPlayerResponse {
                player: comparison.left.player.into(),
                results: response::chart_result_responses(comparison.left.results),
            },
            right: ComparedPlayerResponse {
                player: comparison.right.player.into(),
                results: response::chart_result_responses(comparison.right.results),
            },
        }
    }
}

fn invalid_comparison_filter() -> ApiError {
    ApiError::new(
        StatusCode::BAD_REQUEST,
        "invalid_comparison_filter",
        "Choose a valid track and vehicle",
    )
}

fn validate_distinct_players(query: &PlayerComparisonQuery) -> Result<(), ValidationError> {
    if query.left.eq_ignore_ascii_case(&query.right) {
        return Err(ValidationError::new("duplicate_comparison_driver"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comparison_players_must_be_distinct_case_insensitively() {
        let query = PlayerComparisonQuery {
            left: "Driver".to_owned(),
            right: "driver".to_owned(),
            era: "current".to_owned(),
            track: None,
            vehicle: None,
        };
        assert!(query.validate().is_err());
    }

    #[test]
    fn comparison_query_requires_an_era() {
        let query = PlayerComparisonQuery {
            left: "first".to_owned(),
            right: "second".to_owned(),
            era: " ".to_owned(),
            track: None,
            vehicle: None,
        };
        assert!(query.validate().is_err());
    }
    #[sqlx::test]
    #[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
    async fn chart_filters_preserve_partial_pair_and_availability_rules(
        pool: sqlx::PgPool,
    ) -> anyhow::Result<()> {
        let database = sea_orm::SqlxPostgresConnector::from_sqlx_postgres_pool(pool.clone());
        crate::models::Track::sync(&database).await?;
        crate::models::Vehicle::sync_builtin(&database).await?;
        sqlx::raw_sql(include_str!("../../services/validate_hotlap/fixtures.sql"))
            .execute(&pool)
            .await?;
        sqlx::raw_sql("INSERT INTO player (lfs_username, display_name) VALUES ('other', 'Other');
            INSERT INTO chart (era_id, track_id, vehicle_id) SELECT id, 'BL2', 'XRG' FROM era;
            INSERT INTO ranking_chart_membership (era_id, ranking_id, position, chart_id)
            SELECT ranking.era_id, ranking.id, 1, chart.id FROM ranking JOIN chart ON chart.era_id = ranking.era_id
            WHERE chart.track_id = 'BL2';
            UPDATE vehicle SET available = false WHERE id = 'XFG';").execute(&pool).await?;
        let state = ApiState {
            database,
            oauth: None,
            object_store: std::sync::Arc::new(object_store::memory::InMemory::new()),
            hotlaps: Default::default(),
        };
        for (track, vehicle, valid) in [
            (None, None, true),
            (Some("bl1"), None, true),
            (None, Some("xrg"), true),
            (Some("bl2"), Some("xrg"), true),
            (Some("bl1"), Some("xrg"), false),
            (None, Some("xfg"), false),
            (Some("bl1"), Some("xfg"), false),
            (Some("BL3X"), None, false),
            (Some("NOPE"), None, false),
            (None, Some("NOPE"), false),
            (Some(" "), Some(" "), true),
        ] {
            let result = compare(
                Query(PlayerComparisonQuery {
                    left: "worker-test".into(),
                    right: "other".into(),
                    era: "2026-09-23".into(),
                    track: track.map(str::to_owned),
                    vehicle: vehicle.map(str::to_owned),
                }),
                State(state.clone()),
            )
            .await;
            if valid {
                assert!(result.is_ok(), "{track:?}/{vehicle:?}: {result:?}");
            } else {
                let error = result.unwrap_err();
                assert_eq!(error.status, StatusCode::BAD_REQUEST);
                assert_eq!(error.code, "invalid_comparison_filter");
            }
        }
        Ok(())
    }
}
