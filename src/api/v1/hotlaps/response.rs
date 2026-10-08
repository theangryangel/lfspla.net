//! Shared public hotlap schemas and owner-only submission responses.

use crate::era_slug::EraSlug;

use crate::milliseconds::Milliseconds;

use crate::api::v1::PlayerSummary;

use crate::models::{
    Era, Hotlap as HotlapModel,
    badge::PlayerBadge,
    hotlap::{DriverSide, HotlapState, SteeringInput},
    player::PlayerChartResult,
};
use serde::Serialize;
use utoipa::ToSchema;
#[derive(Debug, Clone, Serialize, ToSchema)]
pub(crate) struct RankingContribution {
    pub(crate) id: String,
    pub(crate) title: String,
}

/// A hotlap's player, with era badges when they have been loaded.
#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct HotlapPlayer {
    #[serde(flatten)]
    player: PlayerSummary,
    /// Null when badges have not been loaded for this response.
    #[schema(required)]
    badges: Option<Vec<PlayerBadge>>,
}

/// Private replay metadata, returned only to the upload owner.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub(crate) struct HotlapSubmission {
    pub(crate) raw_vehicle_name: String,
    #[schema(required)]
    pub(crate) mod_version: Option<i16>,
    #[schema(required)]
    pub(crate) original_filename: Option<String>,
    #[schema(required)]
    pub(crate) hlvc_result_code: Option<i16>,
    #[schema(required)]
    pub(crate) error_detail: Option<String>,
}

/// One hotlap, shared by collections, charts, profiles, comparisons and owner endpoints.
#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct Hotlap {
    pub(crate) id: i64,
    /// Public era slug, used in URLs.
    pub(crate) era_id: EraSlug,
    pub(crate) track: String,
    pub(crate) vehicle: String,
    pub(crate) lap_time_ms: Milliseconds,
    #[serde(with = "time::serde::rfc3339")]
    #[schema(value_type = String, format = DateTime)]
    pub(crate) created_at: time::OffsetDateTime,
    pub(crate) game_version: String,
    /// Relative download URL; null when no replay is stored.
    #[schema(required)]
    pub(crate) replay_url: Option<String>,
    pub(crate) split_1_ms: Milliseconds,
    pub(crate) split_2_ms: Milliseconds,
    pub(crate) split_3_ms: Milliseconds,
    pub(crate) split_4_ms: Milliseconds,
    pub(crate) steering: SteeringInput,
    pub(crate) brake_help_enabled: bool,
    pub(crate) automatic_gears: bool,
    #[schema(required)]
    pub(crate) manual_shifter: Option<bool>,
    pub(crate) axis_clutch: bool,
    pub(crate) automatic_clutch: bool,
    pub(crate) driver_side: DriverSide,
    #[schema(required)]
    pub(crate) abs_enabled: Option<bool>,
    pub(crate) era_title: String,
    pub(crate) player: HotlapPlayer,
    pub(crate) state: HotlapState,
    /// Current chart position; null when unranked or chart data has not been loaded.
    #[schema(required)]
    pub(crate) position: Option<i64>,
    /// Gap to the current world record; null when unranked or chart data has not been loaded.
    #[schema(required)]
    pub(crate) distance_to_world_record_ms: Option<Milliseconds>,
    /// Gap to the benchmark for the requested chart/ranking context; null outside that context.
    #[schema(required)]
    pub(crate) distance_to_benchmark_ms: Option<Milliseconds>,
    /// Rankings containing this lap's chart; null when membership has not been loaded.
    #[schema(required)]
    pub(crate) contributes_to: Option<Vec<RankingContribution>>,
    /// Private metadata omitted unless the endpoint is returning the caller's own upload.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) submission: Option<HotlapSubmission>,
}

impl Hotlap {
    pub(crate) fn new(hotlap: &HotlapModel, era: &Era, player: PlayerSummary) -> Self {
        let controls = hotlap.controls();
        Self {
            split_1_ms: hotlap.split_1_ms,
            split_2_ms: hotlap.split_2_ms,
            split_3_ms: hotlap.split_3_ms,
            split_4_ms: hotlap.split_4_ms,
            steering: controls.steering,
            brake_help_enabled: controls.brake_help_enabled,
            automatic_gears: controls.automatic_gears,
            manual_shifter: controls.manual_shifter,
            axis_clutch: controls.axis_clutch,
            automatic_clutch: controls.automatic_clutch,
            driver_side: controls.driver_side,
            abs_enabled: hotlap.abs_enabled,
            id: hotlap.id,
            era_id: era.slug.clone(),
            era_title: era.title.clone(),
            player: HotlapPlayer {
                player,
                badges: None,
            },
            state: hotlap.state,
            position: None,
            distance_to_world_record_ms: None,
            distance_to_benchmark_ms: None,
            contributes_to: None,
            submission: None,
            track: hotlap.track.to_string(),
            vehicle: hotlap.vehicle.to_string(),
            lap_time_ms: hotlap.lap_time_ms,
            created_at: hotlap.created_at,
            game_version: hotlap.game_version.to_string(),
            replay_url: hotlap
                .spr_object_key
                .as_ref()
                .map(|_| super::replay::download_url(hotlap.id)),
        }
    }
    pub(crate) fn with_submission(mut self, hotlap: &HotlapModel) -> Self {
        self.submission = Some(HotlapSubmission {
            raw_vehicle_name: hotlap.raw_vehicle_name.clone(),
            mod_version: hotlap.mod_version,
            original_filename: hotlap.original_filename.clone(),
            hlvc_result_code: hotlap.hlvc_result_code,
            error_detail: hotlap.error_detail.clone(),
        });
        self
    }

    pub(crate) fn with_badges(mut self, badges: Vec<PlayerBadge>) -> Self {
        self.player.badges = Some(badges);
        self
    }

    pub(crate) fn from_best(
        best: crate::models::chart::BestHotlap,
        badges: Vec<PlayerBadge>,
        era: &Era,
    ) -> Self {
        let mut response = Self::new(&best.hotlap, era, best.player.into()).with_badges(badges);
        response.position = Some(best.position);
        response.distance_to_benchmark_ms = Some(best.distance_to_benchmark_ms);
        response.distance_to_world_record_ms = Some(best.distance_to_world_record_ms);
        response
    }

    pub(crate) fn from_chart_result(result: PlayerChartResult, player: PlayerSummary) -> Self {
        let controls = crate::models::hotlap::ControlConfiguration::from_player_flags(
            result.player_flags.0,
            &result.game_version.0,
        );
        Self {
            id: result.hotlap_id,
            era_id: result.era_slug,
            era_title: result.era_title,
            player: HotlapPlayer {
                player,
                badges: None,
            },
            track: result.track,
            vehicle: result.vehicle,
            lap_time_ms: result.lap_time_ms,
            created_at: result.created_at,
            game_version: result.game_version.to_string(),
            replay_url: result
                .downloadable
                .then(|| super::replay::download_url(result.hotlap_id)),
            split_1_ms: result.split_1_ms,
            split_2_ms: result.split_2_ms,
            split_3_ms: result.split_3_ms,
            split_4_ms: result.split_4_ms,
            steering: controls.steering,
            brake_help_enabled: controls.brake_help_enabled,
            automatic_gears: controls.automatic_gears,
            manual_shifter: controls.manual_shifter,
            axis_clutch: controls.axis_clutch,
            automatic_clutch: controls.automatic_clutch,
            driver_side: controls.driver_side,
            abs_enabled: result.abs_enabled,
            state: HotlapState::Valid,
            position: Some(result.position),
            distance_to_world_record_ms: Some(result.distance_to_world_record_ms),
            distance_to_benchmark_ms: None,
            contributes_to: None,
            submission: None,
        }
    }
}

/// Shared conversion for model pages; private metadata is authorized by the caller.
pub(crate) fn list_response(
    page: crate::models::hotlap::HotlapPage,
    pagination: &crate::api::PaginationQuery,
    submission_owner: Option<i64>,
) -> crate::api::PaginatedResponse<Hotlap> {
    crate::api::PaginatedResponse {
        items: page
            .entries
            .into_iter()
            .map(|entry| {
                let crate::models::hotlap::HotlapListEntry {
                    hotlap,
                    player,
                    era,
                    position,
                    distance_to_world_record_ms,
                    distance_to_benchmark_ms,
                    contributes_to,
                    badges,
                } = entry;
                let mut response = Hotlap::new(&hotlap, &era, player.into());
                response.position = position;
                response.distance_to_world_record_ms = distance_to_world_record_ms;
                response.distance_to_benchmark_ms = distance_to_benchmark_ms;
                response.contributes_to =
                    Some(contributes_to.into_iter().map(Into::into).collect());
                if let Some(badges) = badges {
                    response = response.with_badges(badges);
                }
                if submission_owner == Some(hotlap.player_id) {
                    response = response.with_submission(&hotlap);
                }
                response
            })
            .collect(),
        pagination: pagination.metadata(page.total),
    }
}

impl From<crate::models::hotlap::HotlapListError> for crate::api::ApiError {
    fn from(error: crate::models::hotlap::HotlapListError) -> Self {
        use crate::models::hotlap::HotlapListError;
        match error {
            HotlapListError::Database(error) => Self::database(error),
            HotlapListError::MissingPlayer => Self::new(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "missing_player",
                "Hotlap owner could not be loaded.",
            ),
            HotlapListError::MissingEra => Self::new(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "missing_era",
                "Hotlap era could not be loaded.",
            ),
        }
    }
}

impl From<crate::models::hotlap::RankingContribution> for RankingContribution {
    fn from(value: crate::models::hotlap::RankingContribution) -> Self {
        Self {
            id: value.id,
            title: value.title,
        }
    }
}
