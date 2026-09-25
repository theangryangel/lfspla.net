//! Response types and conversions shared by player routes.

use serde::Serialize;
use utoipa::ToSchema;

use crate::{api::ApiError, models::players::PlayerChartResult};

/// A high-ranking current personal best on an individual chart.
#[derive(Debug, Serialize, ToSchema)]
pub(in crate::api::v1) struct PlayerChartResultResponse {
    hotlap_id: i64,
    /// Relative URL of the original replay, absent when none is stored.
    #[schema(required)]
    spr_url: Option<String>,
    era_id: String,
    track: String,
    vehicle: String,
    lap_time_ms: i64,
    distance_to_world_record_ms: i64,
    position: i64,
    entries: i64,
    #[serde(with = "time::serde::rfc3339")]
    #[schema(value_type = String, format = DateTime)]
    created_at: time::OffsetDateTime,
    game_version: String,
}

pub(in crate::api::v1) fn chart_result_responses(
    results: Vec<PlayerChartResult>,
) -> Result<Vec<PlayerChartResultResponse>, ApiError> {
    results
        .into_iter()
        .map(|result| {
            Ok(PlayerChartResultResponse {
                hotlap_id: result.hotlap_id,
                spr_url: result
                    .downloadable
                    .then(|| crate::api::v1::hotlaps::replay::download_url(result.hotlap_id)),
                era_id: result.era_slug,
                track: result.track,
                vehicle: result.vehicle,
                lap_time_ms: result.lap_time_ms,
                distance_to_world_record_ms: result.distance_to_world_record_ms,
                position: result.position,
                entries: result.entries,
                created_at: result.created_at,
                game_version: result.game_version,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chart_results_serialize_the_public_era_slug() {
        let results = chart_result_responses(vec![PlayerChartResult {
            hotlap_id: 42,
            downloadable: false,
            era_id: 987,
            era_slug: "2007-12-21".to_owned(),
            track: "BL1".to_owned(),
            vehicle: "XFG".to_owned(),
            lap_time_ms: 90_000,
            distance_to_world_record_ms: 100,
            position: 2,
            entries: 10,
            created_at: time::OffsetDateTime::UNIX_EPOCH,
            game_version: "0.5Y".to_owned(),
        }])
        .unwrap();

        let json = serde_json::to_value(results).unwrap();
        assert_eq!(json[0]["era_id"], "2007-12-21");
    }
}
