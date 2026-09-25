//! Comparison of two players' current personal bests.

use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
};
use insim_core::{track::Track, vehicle::Vehicle};
use sea_orm::EntityTrait;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::{router::OpenApiRouter, routes};
use validator::{Validate, ValidationError};

use crate::{
    api::{
        ApiError, ApiState, ErrorResponse, extractors as extract,
        v1::{PlayerSummary, players::response},
    },
    models::{
        hotlaps::HotlapRankable,
        players::{PlayerComparison, PlayerEntity, PlayerFilter},
    },
};

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
            value
                .to_ascii_uppercase()
                .parse::<Vehicle>()
                .expect("insim_core vehicle parsing is infallible")
                .ensure_hotlap_rankable()
        })
        .transpose()
        .map_err(|_| invalid_comparison_filter())?;
    let era_id = query.era.trim();
    let era = extract::resolve_era(&state.database, era_id).await?;
    let track_eligible = match &track {
        Some(track) => era
            .admits_track(&state.database, track)
            .await
            .map_err(ApiError::database)?,
        None => true,
    };
    let vehicle_eligible = match &vehicle {
        Some(vehicle) => era
            .admits_vehicle(&state.database, vehicle)
            .await
            .map_err(ApiError::database)?,
        None => true,
    };
    let pair_eligible = match (&track, &vehicle) {
        (Some(track), Some(vehicle)) => era
            .admits_combination(&state.database, track, vehicle)
            .await
            .map_err(ApiError::database)?,
        _ => true,
    };
    if !track_eligible || !vehicle_eligible || !pair_eligible {
        return Err(invalid_comparison_filter());
    }
    let track = track.map(|track| track.to_string());
    let vehicle = vehicle.map(|vehicle| vehicle.to_string());
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

    Ok(Json(PlayerComparisonResponse::try_from(comparison)?))
}

impl TryFrom<PlayerComparison> for PlayerComparisonResponse {
    type Error = ApiError;

    fn try_from(comparison: PlayerComparison) -> Result<Self, Self::Error> {
        Ok(Self {
            left: ComparedPlayerResponse {
                player: comparison.left.player.into(),
                results: response::chart_result_responses(comparison.left.results)?,
            },
            right: ComparedPlayerResponse {
                player: comparison.right.player.into(),
                results: response::chart_result_responses(comparison.right.results)?,
            },
        })
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
}
