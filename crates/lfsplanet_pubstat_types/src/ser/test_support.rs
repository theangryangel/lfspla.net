//! Shared loading of real capture fixtures for serializer regression tests.
use crate::{Lap, track::TrackPubstatExt};
use insim_core::track::Track;

/// Build normalized data from a real JSON export; the text sibling was
/// captured independently, so this checks field selection, order, and types.
pub(super) fn captured_laps(json: &str, action: &str) -> Vec<Lap> {
    let rows: Vec<serde_json::Value> = serde_json::from_str(json).unwrap();
    rows.iter()
        .map(|row| {
            let track = row
                .get("track")
                .map_or("000", |value| value.as_str().unwrap());
            Lap {
                id: row[if action == "wr" { "id_wr" } else { "id_hl" }]
                    .as_i64()
                    .unwrap(),
                track: Track::from_pubstat_id(track.parse().unwrap()).unwrap(),
                car: serde_json::from_value(
                    row.get("car").cloned().unwrap_or_else(|| "XFG".into()),
                )
                .unwrap(),
                split1: row["split1"].as_i64().unwrap(),
                split2: row["split2"].as_i64().unwrap(),
                split3: row["split3"].as_i64().unwrap(),
                laptime: row["laptime"].as_i64().unwrap(),
                flags_hlaps: serde_json::from_value(row["flags_hlaps"].clone()).unwrap(),
                racername: row
                    .get("racername")
                    .map_or("", |value| value.as_str().unwrap())
                    .into(),
                timestamp: row
                    .get("timestamp")
                    .map_or(0, |value| value.as_i64().unwrap()),
            }
        })
        .collect()
}
