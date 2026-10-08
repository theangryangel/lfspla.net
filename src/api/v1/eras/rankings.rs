//! Rankings offered by an era, and their standings.

use crate::milliseconds::Milliseconds;

use super::response::Chart;
use crate::api::v1::hotlaps::response::Hotlap;
use crate::{
    api::{
        ApiError, ApiState, ErrorResponse, ListResponse, extractors as extract,
        extractors::AuthenticatedPlayer, v1::PlayerSummary,
    },
    models::{
        badge::PlayerBadge,
        chart::BestHotlap,
        ranking::{self, RankingFilter, RankingRules},
    },
};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use sea_orm::{EntityTrait, QueryOrder};
use serde::Serialize;
use utoipa::ToSchema;
use utoipa_axum::{router::OpenApiRouter, routes};

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

impl From<crate::models::Ranking> for RankingSummary {
    fn from(ranking: crate::models::Ranking) -> Self {
        Self {
            id: ranking.slug.to_string(),
            title: ranking.title,
            description: ranking.description,
            my_progress: None,
        }
    }
}

/// One required track and vehicle combination in a ranking.
#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct RankingChart {
    chart: Chart,
    /// The signed-in player's fastest validated lap for this combination.
    #[schema(required)]
    my_hotlap: Option<Hotlap>,
}

impl RankingChart {
    fn new(chart: Chart, my_hotlap: Option<Hotlap>) -> Self {
        Self { chart, my_hotlap }
    }
}

/// Completion of the required charts by the signed-in player.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub(crate) struct RankingProgressResponse {
    total_charts: usize,
    completed_charts: usize,
}

impl RankingProgressResponse {
    fn new(total_charts: i64, completed_charts: i64) -> Self {
        Self {
            total_charts: usize::try_from(total_charts).unwrap_or(usize::MAX),
            completed_charts: usize::try_from(completed_charts).unwrap_or(usize::MAX),
        }
    }
}

/// Complete metadata for one configured ranking instance.
#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct RankingDetailResponse {
    #[serde(flatten)]
    ranking: RankingSummary,
    rules: RankingRules,
    charts: Vec<RankingChart>,
}

/// One row in a personal benchmark-handicap ranking.
#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct PersonalRankingEntryResponse {
    position: i64,
    player: PlayerSummary,
    completed_charts: i64,
    total_charts: usize,
    /// Sum of lap time minus the configured benchmark for each chart.
    handicap_ms: Milliseconds,
    badges: Vec<PlayerBadge>,
}

/// Standings for one configured ranking instance, with a typed entry collection.
#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct RankingStandings<T> {
    id: String,
    title: String,
    total_charts: usize,
    entries: Vec<T>,
}

/// One row in a national points ranking.
#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct NationRankingEntryResponse {
    position: i64,
    country_code: String,
    points: i64,
    handicap_ms: Milliseconds,
    contributing_laps: i64,
    contributing_charts: i64,
}

/// A player's contribution to a country's score in one ranking.
#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct NationContributionResponse {
    player: PlayerSummary,
    points: i64,
    contributing_charts: i64,
    handicap_ms: Milliseconds,
}

impl From<ranking::NationContribution> for NationContributionResponse {
    fn from(contribution: ranking::NationContribution) -> Self {
        Self {
            player: contribution.player.into(),
            points: contribution.points,
            contributing_charts: contribution.contributing_charts,
            handicap_ms: contribution.handicap_ms,
        }
    }
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
    let rankings = ranking::Entity::find()
        .in_era(era.id)
        .order_by_asc(ranking::Column::Position)
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
                    RankingProgressResponse::new(progress.total_charts, progress.completed_charts),
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
    let ranking = ranking::RankingWithCharts::find(&state.database, era.id, &ranking_id)
        .await
        .map_err(ApiError::database)?
        .ok_or_else(|| ApiError::not_found("ranking_not_found", "Ranking"))?;
    let rules = ranking.definition.rules();
    let (mut personal_bests, completed_charts) = match player.as_ref() {
        Some(player) => {
            let personal_bests = ranking
                .definition
                .list_personal_chart_bests(&state.database, player)
                .await
                .map_err(ApiError::database)?;
            let completed_charts = personal_bests.len();
            let badges = era
                .list_badges_for_players(&state.database, [player.id])
                .await
                .map_err(ApiError::database)?
                .remove(&player.id)
                .unwrap_or_default();
            let personal_bests = personal_bests
                .into_iter()
                .map(|best| {
                    let key = best.hotlap.chart_id;
                    let response = Hotlap::from_best(
                        BestHotlap {
                            position: best.position,
                            hotlap: best.hotlap,
                            player: player.clone(),
                            distance_to_benchmark_ms: best.distance_to_benchmark_ms,
                            distance_to_world_record_ms: best.distance_to_world_record_ms,
                        },
                        badges.clone(),
                        &era,
                    );
                    (key, response)
                })
                .collect::<std::collections::HashMap<_, _>>();
            (personal_bests, completed_charts)
        }
        None => (std::collections::HashMap::new(), 0),
    };

    let chart_responses = Chart::load(&state.database, &era, &ranking.charts).await?;

    Ok(Json(RankingDetailResponse {
        ranking: RankingSummary {
            id: ranking.definition.slug.to_string(),
            title: ranking.definition.title,
            description: ranking.definition.description,
            my_progress: player.map(|_| RankingProgressResponse {
                total_charts: ranking.charts.len(),
                completed_charts,
            }),
        },
        rules,
        charts: ranking
            .charts
            .iter()
            .zip(chart_responses)
            .map(|(chart, response)| RankingChart::new(response, personal_bests.remove(&chart.id)))
            .collect(),
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
        (status = 200, description = "Personal ranking standings", body = RankingStandings<PersonalRankingEntryResponse>),
        (status = 404, description = "Era or ranking not found", body = ErrorResponse),
        (status = 409, description = "Ranking chart selection is not configured", body = ErrorResponse)
    )
)]
pub(crate) async fn players(
    Path((era_id, ranking_id)): Path<(String, String)>,
    State(state): State<ApiState>,
) -> Result<Json<RankingStandings<PersonalRankingEntryResponse>>, ApiError> {
    let era = extract::resolve_era(&state.database, &era_id).await?;
    let ranking = crate::models::Ranking::find(&state.database, era.id, &ranking_id)
        .await
        .map_err(ApiError::database)?
        .ok_or_else(|| ApiError::not_found("ranking_not_found", "Ranking"))?;
    let total_charts = ranking
        .chart_count(&state.database)
        .await
        .map_err(ApiError::database)?;
    if total_charts == 0 {
        return Err(ranking_not_configured());
    }
    let rows = ranking
        .personal(&state.database)
        .await
        .map_err(ApiError::database)?;

    Ok(Json(RankingStandings {
        id: ranking.slug.to_string(),
        title: ranking.title,
        total_charts,
        entries: rows
            .into_iter()
            .map(|standing| {
                let row = standing.row;
                PersonalRankingEntryResponse {
                    position: row.position,
                    player: PlayerSummary {
                        id: row.player_id,
                        lfs_username: row.lfs_username,
                        display_name: row.display_name,
                        country_code: row.country_code.map(|code| code.as_str().to_owned()),
                        flag_code: row.flag_code.map(|code| code.as_str().to_owned()),
                    },
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
        (status = 200, description = "National ranking standings", body = RankingStandings<NationRankingEntryResponse>),
        (status = 404, description = "Era or ranking not found", body = ErrorResponse),
        (status = 409, description = "Ranking chart selection is not configured", body = ErrorResponse)
    )
)]
pub(crate) async fn nations(
    Path((era_id, ranking_id)): Path<(String, String)>,
    State(state): State<ApiState>,
) -> Result<Json<RankingStandings<NationRankingEntryResponse>>, ApiError> {
    let era = extract::resolve_era(&state.database, &era_id).await?;
    let ranking = crate::models::Ranking::find(&state.database, era.id, &ranking_id)
        .await
        .map_err(ApiError::database)?
        .ok_or_else(|| ApiError::not_found("ranking_not_found", "Ranking"))?;
    let total_charts = ranking
        .chart_count(&state.database)
        .await
        .map_err(ApiError::database)?;
    if total_charts == 0 {
        return Err(ranking_not_configured());
    }

    let rows = ranking
        .nations(&state.database)
        .await
        .map_err(ApiError::database)?;

    Ok(Json(RankingStandings {
        id: ranking.slug.to_string(),
        title: ranking.title,
        total_charts,
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
        (status = 200, description = "Scoring contributions by player", body = ListResponse<NationContributionResponse>),
        (status = 404, description = "Era or ranking not found", body = ErrorResponse),
        (status = 409, description = "Ranking chart selection is not configured", body = ErrorResponse)
    )
)]
pub(crate) async fn nation_contributions(
    Path((era_id, ranking_id, country)): Path<(String, String, String)>,
    State(state): State<ApiState>,
) -> Result<Json<ListResponse<NationContributionResponse>>, ApiError> {
    let era = extract::resolve_era(&state.database, &era_id).await?;
    let ranking = crate::models::Ranking::find(&state.database, era.id, &ranking_id)
        .await
        .map_err(ApiError::database)?
        .ok_or_else(|| ApiError::not_found("ranking_not_found", "Ranking"))?;
    let total_charts = ranking
        .chart_count(&state.database)
        .await
        .map_err(ApiError::database)?;
    if total_charts == 0 {
        return Err(ranking_not_configured());
    }
    let rows = ranking
        .list_nation_contributions(&state.database, &country.to_ascii_uppercase())
        .await
        .map_err(ApiError::database)?;
    Ok(Json(ListResponse::from(
        rows.into_iter()
            .map(NationContributionResponse::from)
            .collect::<Vec<_>>(),
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standings_and_contributors_serialize_a_shared_nested_player() {
        let player = PlayerSummary {
            id: 7,
            lfs_username: "driver".to_owned(),
            display_name: "Driver".to_owned(),
            country_code: Some("GB".to_owned()),
            flag_code: None,
        };
        let standings = RankingStandings {
            id: "nutter".to_owned(),
            title: "Nutter".to_owned(),
            total_charts: 3,
            entries: vec![PersonalRankingEntryResponse {
                position: 1,
                player: player.clone(),
                completed_charts: 2,
                total_charts: 3,
                handicap_ms: Milliseconds::ZERO,
                badges: vec![],
            }],
        };
        let contributor = NationContributionResponse {
            player: player.clone(),
            points: 10,
            contributing_charts: 2,
            handicap_ms: Milliseconds::ZERO,
        };
        let standings = serde_json::to_value(standings).unwrap();
        let contributor = serde_json::to_value(contributor).unwrap();
        let expected = serde_json::to_value(player).unwrap();
        assert_eq!(standings["id"], "nutter");
        assert!(standings.get("ranking_id").is_none());
        for row in [&standings["entries"][0], &contributor] {
            assert_eq!(row["player"], expected);
            for old in [
                "player_id",
                "lfs_username",
                "display_name",
                "country_code",
                "flag_code",
            ] {
                assert!(row.get(old).is_none());
            }
        }
    }

    #[test]
    fn ranking_detail_keeps_flat_metadata_and_chart_progress() {
        let response = RankingDetailResponse {
            ranking: RankingSummary {
                id: "nutter".to_owned(),
                title: "Nutter".to_owned(),
                description: "Complete all charts".to_owned(),
                my_progress: Some(RankingProgressResponse::new(3, 2)),
            },
            rules: RankingRules {
                benchmark_percent: 103,
                nation_max_points: 10,
                nation_driver_limit: 3,
            },
            charts: vec![],
        };
        let json = serde_json::to_value(response).unwrap();
        assert_eq!(json["id"], "nutter");
        assert!(json.get("ranking").is_none());
        assert_eq!(
            json["my_progress"],
            serde_json::json!({ "total_charts": 3, "completed_charts": 2 })
        );
    }
}
