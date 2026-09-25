//! Response models shared by the routes that manage uploaded hotlaps.

use serde::Serialize;
use utoipa::ToSchema;

use crate::models::{
    eras::EraModel,
    hotlaps::{DriverSide, HotlapModel, HotlapState, SteeringInput},
};

#[derive(Debug, Clone, Serialize, ToSchema)]
pub(crate) struct RankingContribution {
    pub(crate) id: String,
    pub(crate) title: String,
}

/// One of the current player's uploaded hotlaps, in any lifecycle state.
#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct ManagedHotlapResponse {
    id: i64,
    player_id: i64,
    /// Public era slug, used in URLs.
    era_id: String,
    era_title: String,
    track: String,
    #[schema(required)]
    vehicle: Option<String>,
    raw_vehicle_name: String,
    #[schema(required)]
    mod_version: Option<i16>,
    lap_time_ms: i64,
    /// Current chart position when loaded from the hotlap collection.
    #[schema(required)]
    position: Option<i64>,
    /// Gap to the current chart world record in milliseconds.
    #[schema(required)]
    distance_to_world_record_ms: Option<i64>,
    contributes_to: Vec<RankingContribution>,
    split_1_ms: i64,
    split_2_ms: i64,
    split_3_ms: i64,
    split_4_ms: i64,
    #[schema(required)]
    original_filename: Option<String>,
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
    state: HotlapState,
    #[schema(required)]
    hlvc_result_code: Option<i16>,
    #[schema(required)]
    error_detail: Option<String>,
    /// Download URL for this stored replay.
    #[schema(required)]
    replay_url: Option<String>,
}

impl ManagedHotlapResponse {
    pub(crate) fn new(hotlap: HotlapModel, era: &EraModel) -> Self {
        let controls = hotlap.controls();
        Self {
            id: hotlap.id,
            player_id: hotlap.player_id,
            era_id: era.slug.clone(),
            era_title: era.title.clone(),
            track: hotlap.track.to_string(),
            vehicle: hotlap.vehicle.map(|vehicle| vehicle.to_string()),
            raw_vehicle_name: hotlap.raw_vehicle_name,
            mod_version: hotlap.mod_version,
            lap_time_ms: hotlap.lap_time_ms,
            position: None,
            distance_to_world_record_ms: None,
            contributes_to: Vec::new(),
            split_1_ms: hotlap.split_1_ms,
            split_2_ms: hotlap.split_2_ms,
            split_3_ms: hotlap.split_3_ms,
            split_4_ms: hotlap.split_4_ms,
            original_filename: hotlap.original_filename,
            steering: controls.steering,
            brake_help_enabled: controls.brake_help_enabled,
            automatic_gears: controls.automatic_gears,
            manual_shifter: controls.manual_shifter,
            axis_clutch: controls.axis_clutch,
            automatic_clutch: controls.automatic_clutch,
            driver_side: controls.driver_side,
            abs_enabled: hotlap.abs_enabled,
            created_at: hotlap.created_at,
            game_version: hotlap.game_version.to_string(),
            state: hotlap.state,
            hlvc_result_code: hotlap.hlvc_result_code,
            error_detail: hotlap.error_detail,
            replay_url: hotlap
                .spr_object_key
                .as_ref()
                .map(|_| crate::api::v1::hotlaps::replay::download_url(hotlap.id)),
        }
    }

    pub(crate) fn with_chart_data(
        mut self,
        position: i64,
        distance_to_world_record_ms: i64,
    ) -> Self {
        self.position = Some(position);
        self.distance_to_world_record_ms = Some(distance_to_world_record_ms);
        self
    }

    pub(crate) fn with_ranking_contributions(
        mut self,
        contributions: Vec<RankingContribution>,
    ) -> Self {
        self.contributes_to = contributions;
        self
    }
}
