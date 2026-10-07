//! Conversion of player chart results to the shared hotlap response.

use crate::api::v1::{PlayerSummary, hotlaps::response::Hotlap};
use crate::models::player::PlayerChartResult;

pub(in crate::api::v1) fn chart_result_responses(
    results: Vec<PlayerChartResult>,
    player: PlayerSummary,
) -> Vec<Hotlap> {
    results
        .into_iter()
        .map(|result| Hotlap::from_chart_result(result, player.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::milliseconds::Milliseconds;

    #[test]
    fn chart_results_serialize_the_public_era_slug() {
        let results = chart_result_responses(
            vec![PlayerChartResult {
                hotlap_id: 42,
                downloadable: false,
                era_id: 987,
                era_slug: "2007-12-21".parse().unwrap(),
                era_title: "Historical".to_owned(),
                track: "BL1".to_owned(),
                vehicle: "XFG".to_owned(),
                lap_time_ms: Milliseconds::from_millis(90_000),
                distance_to_world_record_ms: Milliseconds::from_millis(100),
                position: 2,
                created_at: time::OffsetDateTime::UNIX_EPOCH,
                game_version: crate::game_version::GameVersionCode("0.5Y".parse().unwrap()),
                split_1_ms: Milliseconds::from_millis(30_000),
                split_2_ms: Milliseconds::from_millis(60_000),
                split_3_ms: Milliseconds::from_millis(0),
                split_4_ms: Milliseconds::from_millis(0),
                player_flags: lfsplanet_spr::PlayerFlags::MOUSE.into(),
                abs_enabled: None,
            }],
            PlayerSummary {
                id: 7,
                lfs_username: "driver".to_owned(),
                display_name: "Driver".to_owned(),
                country_code: None,
                flag_code: None,
            },
        );

        let json = serde_json::to_value(results).unwrap();
        assert_eq!(json[0]["era_id"], "2007-12-21");
        assert_eq!(json[0]["id"], 42);
        assert_eq!(json[0]["player"]["id"], 7);
        assert_eq!(json[0]["position"], 2);
        assert!(json[0].get("entries").is_none());
        assert!(json[0].get("submission").is_none());
        assert_eq!(json[0]["split_1_ms"], 30_000);
        assert_eq!(json[0]["split_2_ms"], 60_000);
        assert_eq!(json[0]["steering"], "mouse");
        assert_eq!(json[0]["abs_enabled"], serde_json::Value::Null);
        assert_eq!(json[0]["manual_shifter"], false);
        assert_eq!(json[0]["replay_url"], serde_json::Value::Null);
        assert_eq!(json[0]["created_at"], "1970-01-01T00:00:00Z");
        assert!(json[0].get("hotlap").is_none());
        assert!(json[0].get("hotlap_id").is_none());
        assert!(json[0].get("spr_url").is_none());
    }
}
