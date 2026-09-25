//! Public profile detail for one player.

use axum::{Json, extract::State};
use serde::Serialize;
use utoipa::ToSchema;

use crate::{
    api::{ApiError, ApiState, ErrorResponse, extractors as extract},
    models::{badges::PlayerBadge, players::PlayerProfile},
};

use super::response::{PlayerChartResultResponse, chart_result_responses};

/// Lifetime totals across all stored physics eras.
#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct PlayerStatsResponse {
    hotlaps: i64,
    personal_bests: i64,
    world_records: i64,
    podiums: i64,
    eras: usize,
    #[serde(with = "time::serde::rfc3339::option")]
    #[schema(value_type = Option<String>, format = DateTime, required)]
    first_hotlap_at: Option<time::OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    #[schema(value_type = Option<String>, format = DateTime, required)]
    latest_hotlap_at: Option<time::OffsetDateTime>,
}

/// One era in which the player has a validated lap.
#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct PlayerEraStatsResponse {
    id: String,
    title: String,
    hotlaps: i64,
    personal_bests: i64,
    firsts: i64,
    seconds: i64,
    thirds: i64,
    #[serde(with = "time::serde::rfc3339")]
    #[schema(value_type = String, format = DateTime)]
    first_hotlap_at: time::OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    #[schema(value_type = String, format = DateTime)]
    latest_hotlap_at: time::OffsetDateTime,
    badges: Vec<PlayerBadge>,
}

/// Public profile and competitive record for one LFS account.
#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct PlayerResponse {
    id: i64,
    lfs_username: String,
    display_name: String,
    #[schema(required)]
    country_code: Option<String>,
    stats: PlayerStatsResponse,
    eras: Vec<PlayerEraStatsResponse>,
    highlights: Vec<PlayerChartResultResponse>,
}

#[utoipa::path(
    get,
    path = "/api/v1/players/{lfs_username}",
    operation_id = "get_player",
    tag = "players",
    params(("lfs_username" = String, Path, description = "Stable LFS account name")),
    responses(
        (status = 200, description = "Public player profile", body = PlayerResponse),
        (status = 404, description = "Player not found", body = ErrorResponse)
    )
)]
pub(crate) async fn detail(
    extract::Player(player): extract::Player,
    State(state): State<ApiState>,
) -> Result<Json<PlayerResponse>, ApiError> {
    let profile = player
        .profile(&state.database)
        .await
        .map_err(ApiError::database)?;
    Ok(Json(PlayerResponse::try_from(profile)?))
}

impl TryFrom<PlayerProfile> for PlayerResponse {
    type Error = ApiError;

    fn try_from(profile: PlayerProfile) -> Result<Self, Self::Error> {
        Ok(PlayerResponse {
            id: profile.player.id,
            lfs_username: profile.player.lfs_username,
            display_name: profile.player.display_name,
            country_code: profile
                .player
                .country_code
                .map(|country_code| country_code.as_str().to_owned()),
            stats: PlayerStatsResponse {
                hotlaps: profile.stats.hotlaps,
                personal_bests: profile.stats.personal_bests,
                world_records: profile.stats.world_records,
                podiums: profile.stats.podiums,
                eras: profile.stats.eras,
                first_hotlap_at: profile.stats.first_hotlap_at,
                latest_hotlap_at: profile.stats.latest_hotlap_at,
            },
            eras: profile
                .eras
                .into_iter()
                .map(|era| PlayerEraStatsResponse {
                    id: era.id,
                    title: era.title,
                    hotlaps: era.hotlaps,
                    personal_bests: era.personal_bests,
                    firsts: era.firsts,
                    seconds: era.seconds,
                    thirds: era.thirds,
                    first_hotlap_at: era.first_hotlap_at,
                    latest_hotlap_at: era.latest_hotlap_at,
                    badges: era.badges,
                })
                .collect(),
            highlights: chart_result_responses(profile.highlights)?,
        })
    }
}
