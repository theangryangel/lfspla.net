//! Rankings offered by an era, and their standings.

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use sea_orm::{EntityTrait, QueryOrder};
use serde::Serialize;
use utoipa::ToSchema;
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::{
    api::{
        ApiError, ApiState, ErrorResponse, ListResponse,
        extractors::{self as extract, AuthenticatedPlayer},
    },
    models::{
        badges::PlayerBadge,
        hotlaps::BestHotlap,
        rankings::{self, RankingFilter, RankingRules},
    },
};

use super::charts::BestHotlapResponse;

/// A ranking offered by an era.
#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct RankingSummary {
    id: String,
    title: String,
    description: String,
    /// Present only for authenticated requests to the rankings collection.
    #[schema(required)]
    my_progress: Option<RankingProgressResponse>,
}

impl From<rankings::RankingRow> for RankingSummary {
    fn from(ranking: rankings::RankingRow) -> Self {
        Self {
            id: ranking.slug,
            title: ranking.title,
            description: ranking.description,
            my_progress: None,
        }
    }
}

/// One required track and vehicle combination in a ranking.
#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct RankingCombinationResponse {
    track: String,
    vehicle: String,
    /// The signed-in player's fastest validated lap for this combination.
    #[schema(required)]
    my_hotlap: Option<BestHotlapResponse>,
}

impl RankingCombinationResponse {
    fn new(
        chart: &crate::models::rankings::RankingChartModel,
        my_hotlap: Option<BestHotlapResponse>,
    ) -> Self {
        Self {
            track: chart.track_id.clone(),
            vehicle: chart.vehicle_id.clone(),
            my_hotlap,
        }
    }
}

/// Completion of the required combinations by the signed-in player.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub(crate) struct RankingProgressResponse {
    total_combinations: usize,
    completed_combinations: usize,
}

impl RankingProgressResponse {
    fn new(total_combinations: i64, completed_combinations: i64) -> Self {
        Self {
            total_combinations: usize::try_from(total_combinations).unwrap_or(usize::MAX),
            completed_combinations: usize::try_from(completed_combinations).unwrap_or(usize::MAX),
        }
    }
}

/// Complete metadata for one configured ranking instance.
#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct RankingDetailResponse {
    id: String,
    title: String,
    description: String,
    rules: RankingRules,
    charts: Vec<RankingCombinationResponse>,
    /// Present only for authenticated requests.
    #[schema(required)]
    my_progress: Option<RankingProgressResponse>,
}

/// One row in a personal benchmark-handicap ranking.
#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct PersonalRankingEntryResponse {
    position: i64,
    player_id: i64,
    lfs_username: String,
    display_name: String,
    #[schema(required)]
    country_code: Option<String>,
    completed_charts: i64,
    total_charts: usize,
    /// Sum of lap time minus the configured benchmark for each chart.
    handicap_ms: i64,
    badges: Vec<PlayerBadge>,
}

/// Personal standings for one configured ranking instance.
#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct PersonalRankingResponse {
    ranking_id: String,
    title: String,
    total_charts: usize,
    entries: Vec<PersonalRankingEntryResponse>,
}

/// One row in a national points ranking.
#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct NationRankingEntryResponse {
    position: i64,
    country_code: String,
    points: i64,
    handicap_ms: i64,
    contributing_laps: i64,
    contributing_charts: i64,
}

/// National standings for one configured ranking instance.
#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct NationRankingResponse {
    ranking_id: String,
    title: String,
    total_charts: usize,
    entries: Vec<NationRankingEntryResponse>,
}

/// Builds era-scoped ranking routes.
pub(super) fn router() -> OpenApiRouter<ApiState> {
    OpenApiRouter::new()
        .routes(routes!(list))
        .routes(routes!(detail))
        .routes(routes!(players))
        .routes(routes!(nations))
        .routes(routes!(nation_contributions))
}

#[utoipa::path(
    get,
    path = "/api/v1/eras/{era}/rankings",
    operation_id = "list_era_rankings",
    tag = "ranking",
    params(("era" = String, Path, description = "Era slug")),
    responses(
        (status = 200, description = "Rankings configured for the era", body = ListResponse<RankingSummary>),
        (status = 404, description = "Era was not found", body = ErrorResponse)
    )
)]
pub(crate) async fn list(
    extract::Era(era): extract::Era,
    player: Option<AuthenticatedPlayer>,
    State(state): State<ApiState>,
) -> Result<Json<ListResponse<RankingSummary>>, ApiError> {
    let player = player.map(|AuthenticatedPlayer(player)| player);
    let rankings = rankings::RankingEntity::find()
        .in_era(era.id)
        .order_by_asc(rankings::RankingColumn::Position)
        .all(&state.database)
        .await
        .map_err(ApiError::database)?;
    let authenticated = player.is_some();
    let progress_by_ranking = match player {
        Some(player) => era
            .list_ranking_progresses(&state.database, player.id)
            .await
            .map_err(ApiError::database)?
            .into_iter()
            .map(|progress| {
                (
                    progress.ranking_id,
                    RankingProgressResponse::new(
                        progress.total_combinations,
                        progress.completed_combinations,
                    ),
                )
            })
            .collect::<std::collections::HashMap<_, _>>(),
        None => std::collections::HashMap::new(),
    };
    Ok(Json(ListResponse::from(
        rankings
            .into_iter()
            .map(|ranking| RankingSummary {
                my_progress: authenticated.then(|| {
                    progress_by_ranking
                        .get(&ranking.id)
                        .cloned()
                        .unwrap_or_else(|| RankingProgressResponse::new(0, 0))
                }),
                ..ranking.into()
            })
            .collect::<Vec<_>>(),
    )))
}

#[utoipa::path(
    get,
    path = "/api/v1/eras/{era}/rankings/{ranking}",
    operation_id = "get_era_ranking",
    tag = "ranking",
    params(
        ("era" = String, Path, description = "Target era identifier"),
        ("ranking" = String, Path, description = "Ranking identifier")
    ),
    responses(
        (status = 200, description = "Ranking rules and required charts", body = RankingDetailResponse),
        (status = 404, description = "Era or ranking not found", body = ErrorResponse)
    )
)]
pub(crate) async fn detail(
    Path((era_id, ranking_id)): Path<(String, String)>,
    player: Option<AuthenticatedPlayer>,
    State(state): State<ApiState>,
) -> Result<Json<RankingDetailResponse>, ApiError> {
    let player = player.map(|AuthenticatedPlayer(player)| player);
    let era = extract::resolve_era(&state.database, &era_id).await?;
    let ranking = rankings::LoadedRanking::find(&state.database, era.id, &ranking_id)
        .await
        .map_err(ApiError::database)?
        .ok_or_else(|| ApiError::not_found("ranking_not_found", "Ranking"))?;
    let rules = ranking.rules();
    let (mut personal_bests, completed_combinations) = match player.as_ref() {
        Some(player) => {
            let personal_bests = ranking
                .list_personal_chart_bests(&state.database, player)
                .await
                .map_err(ApiError::database)?;
            let completed_combinations = personal_bests.len();
            let badges = era
                .list_badges_for_players(&state.database, [player.id])
                .await
                .map_err(ApiError::database)?
                .remove(&player.id)
                .unwrap_or_default();
            let personal_bests = personal_bests
                .into_iter()
                .map(|best| {
                    let key = (best.track, best.vehicle);
                    let response = BestHotlapResponse::from_best(
                        BestHotlap {
                            position: best.position,
                            hotlap: best.hotlap,
                            player: player.clone(),
                            distance_to_benchmark_ms: best.distance_to_benchmark_ms,
                            distance_to_world_record_ms: best.distance_to_world_record_ms,
                        },
                        badges.clone(),
                    )?;
                    Ok((key, response))
                })
                .collect::<Result<std::collections::HashMap<_, _>, _>>()
                .map_err(ApiError::database)?;
            (personal_bests, completed_combinations)
        }
        None => (std::collections::HashMap::new(), 0),
    };

    Ok(Json(RankingDetailResponse {
        id: ranking.definition.slug,
        title: ranking.definition.title,
        description: ranking.definition.description,
        rules,
        charts: ranking
            .charts
            .iter()
            .map(|chart| {
                let key = (chart.track_id.clone(), chart.vehicle_id.clone());
                RankingCombinationResponse::new(chart, personal_bests.remove(&key))
            })
            .collect(),
        my_progress: player.map(|_| RankingProgressResponse {
            total_combinations: ranking.charts.len(),
            completed_combinations,
        }),
    }))
}

#[utoipa::path(
    get,
    path = "/api/v1/eras/{era}/rankings/{ranking}/players",
    tag = "ranking",
    params(
        ("era" = String, Path, description = "Target era identifier"),
        ("ranking" = String, Path, description = "Ranking identifier")
    ),
    responses(
        (status = 200, description = "Personal ranking standings", body = PersonalRankingResponse),
        (status = 404, description = "Era or ranking not found", body = ErrorResponse),
        (status = 409, description = "Ranking chart selection is not configured", body = ErrorResponse)
    )
)]
pub(crate) async fn players(
    Path((era_id, ranking_id)): Path<(String, String)>,
    State(state): State<ApiState>,
) -> Result<Json<PersonalRankingResponse>, ApiError> {
    let era = extract::resolve_era(&state.database, &era_id).await?;
    let ranking = rankings::LoadedRanking::find(&state.database, era.id, &ranking_id)
        .await
        .map_err(ApiError::database)?
        .ok_or_else(|| ApiError::not_found("ranking_not_found", "Ranking"))?;
    if ranking.charts.is_empty() {
        return Err(ranking_not_configured());
    }
    let rows = ranking
        .personal(&state.database, &era)
        .await
        .map_err(ApiError::database)?;
    let total_charts = ranking.charts.len();

    Ok(Json(PersonalRankingResponse {
        ranking_id: ranking.definition.slug,
        title: ranking.definition.title,
        total_charts,
        entries: rows
            .into_iter()
            .map(|standing| {
                let row = standing.row;
                PersonalRankingEntryResponse {
                    position: row.position,
                    player_id: row.player_id,
                    lfs_username: row.lfs_username,
                    display_name: row.display_name,
                    country_code: row.country_code.map(|code| code.as_str().to_owned()),
                    completed_charts: row.completed_charts,
                    total_charts,
                    handicap_ms: row.handicap_ms,
                    badges: standing.badges,
                }
            })
            .collect(),
    }))
}

#[utoipa::path(
    get,
    path = "/api/v1/eras/{era}/rankings/{ranking}/nations",
    tag = "ranking",
    params(
        ("era" = String, Path, description = "Target era identifier"),
        ("ranking" = String, Path, description = "Ranking identifier")
    ),
    responses(
        (status = 200, description = "National ranking standings", body = NationRankingResponse),
        (status = 404, description = "Era or ranking not found", body = ErrorResponse),
        (status = 409, description = "Ranking chart selection is not configured", body = ErrorResponse)
    )
)]
pub(crate) async fn nations(
    Path((era_id, ranking_id)): Path<(String, String)>,
    State(state): State<ApiState>,
) -> Result<Json<NationRankingResponse>, ApiError> {
    let era = extract::resolve_era(&state.database, &era_id).await?;
    let ranking = rankings::LoadedRanking::find(&state.database, era.id, &ranking_id)
        .await
        .map_err(ApiError::database)?
        .ok_or_else(|| ApiError::not_found("ranking_not_found", "Ranking"))?;
    if ranking.charts.is_empty() {
        return Err(ranking_not_configured());
    }

    let rows = ranking
        .nations(&state.database)
        .await
        .map_err(ApiError::database)?;

    Ok(Json(NationRankingResponse {
        ranking_id: ranking.definition.slug,
        title: ranking.definition.title,
        total_charts: ranking.charts.len(),
        entries: rows
            .into_iter()
            .map(|row| NationRankingEntryResponse {
                position: row.position,
                country_code: row.country_code.as_str().to_owned(),
                points: row.points,
                handicap_ms: row.handicap_ms,
                contributing_laps: row.contributing_laps,
                contributing_charts: row.contributing_charts,
            })
            .collect(),
    }))
}

fn ranking_not_configured() -> ApiError {
    ApiError::new(
        StatusCode::CONFLICT,
        "ranking_not_configured",
        "Ranking chart selection is not configured",
    )
}

#[utoipa::path(
    get,
    path = "/api/v1/eras/{era}/rankings/{ranking}/nations/{country}/contributors",
    tag = "ranking",
    params(
        ("era" = String, Path, description = "Era slug"),
        ("ranking" = String, Path, description = "Ranking slug"),
        ("country" = String, Path, description = "Country code")
    ),
    responses(
        (status = 200, description = "Scoring contributions by player", body = ListResponse<rankings::NationContribution>),
        (status = 404, description = "Era or ranking not found", body = ErrorResponse),
        (status = 409, description = "Ranking chart selection is not configured", body = ErrorResponse)
    )
)]
pub(crate) async fn nation_contributions(
    Path((era_id, ranking_id, country)): Path<(String, String, String)>,
    State(state): State<ApiState>,
) -> Result<Json<ListResponse<rankings::NationContribution>>, ApiError> {
    let era = extract::resolve_era(&state.database, &era_id).await?;
    let ranking = rankings::LoadedRanking::find(&state.database, era.id, &ranking_id)
        .await
        .map_err(ApiError::database)?
        .ok_or_else(|| ApiError::not_found("ranking_not_found", "Ranking"))?;
    if ranking.charts.is_empty() {
        return Err(ranking_not_configured());
    }
    let rows = ranking
        .list_nation_contributions(&state.database, &country.to_ascii_uppercase())
        .await
        .map_err(ApiError::database)?;
    Ok(Json(ListResponse::from(rows)))
}
