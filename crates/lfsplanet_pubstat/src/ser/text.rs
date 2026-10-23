//! Text formatting only: scalar values, space-separated fields, and LF rows.
use super::Error;
use std::{fmt, io::Write};

pub(super) struct Text<W> {
    writer: W,
    fields: usize,
}

impl<W: Write> Text<W> {
    pub(super) fn new(writer: W) -> Self {
        Self { writer, fields: 0 }
    }

    pub(super) fn start_row(&mut self) {
        self.fields = 0;
    }

    pub(super) fn scalar(&mut self, value: impl fmt::Display) -> Result<(), Error> {
        if self.fields > 0 {
            self.writer.write_all(b" ")?;
        }
        write!(self.writer, "{value}")?;
        self.fields += 1;
        Ok(())
    }

    pub(super) fn string(&mut self, value: &str) -> Result<(), Error> {
        if value.contains(['\r', '\n', '\0']) {
            return Err(Error::Message(
                "Pubstat fields cannot contain line breaks or NUL".into(),
            ));
        }
        self.scalar(value)
    }

    pub(super) fn end_row(&mut self) -> Result<(), Error> {
        self.writer.write_all(b"\n")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::ser as text;
    use crate::ser::test_support::captured_laps;
    use crate::{Lap, Version, ch::Chart, hl::Hotlaps, track::TrackPubstatExt, wr::WorldRecords};
    use insim_core::{track::Track, vehicle::Vehicle};
    use serde::Serialize;

    fn lap() -> Lap {
        Lap {
            id: 42,
            track: Track::Bl1,
            car: Vehicle::Xfg,
            split1: 10_000,
            split2: 20_000,
            split3: 0,
            laptime: 30_000,
            flags_hlaps: 1025,
            racername: "a racer".into(),
            timestamp: 1_000_000,
        }
    }

    #[test]
    fn every_version_matches_its_documented_column_layout() {
        use Version::{V1_0, V1_1, V1_2, V1_3, V1_4, V1_5};
        let fixtures = [
            (
                V1_0,
                "000 XFG 30000 1025",
                "30000 1025 a racer",
                "000 XFG 30000 1025 a racer",
            ),
            (
                V1_1,
                "000 XFG 30000 1025",
                "30000 1025 a racer",
                "000 XFG 30000 1025 a racer",
            ),
            (
                V1_2,
                "42 000 XFG 30000 1025",
                "30000 1025 a racer",
                "42 000 XFG 30000 1025 a racer",
            ),
            (
                V1_3,
                "42 000 XFG 10000 20000 0 30000 1025",
                "10000 20000 0 30000 1025 a racer",
                "42 000 XFG 10000 20000 0 30000 1025 a racer",
            ),
            (
                V1_4,
                "42 000 XFG 10000 20000 0 30000 1025 1000000",
                "42 10000 20000 0 30000 1025 a racer",
                "42 000 XFG 10000 20000 0 30000 1025 a racer",
            ),
            (
                V1_5,
                "42 000 XFG 10000 20000 0 30000 1025 1000000",
                "42 10000 20000 0 30000 1025 a racer",
                "42 000 XFG 10000 20000 0 30000 1025 a racer 1000000",
            ),
        ];
        for (version, hl, ch, wr) in fixtures {
            assert_eq!(
                text::to_string(&Hotlaps::new(version, vec![lap()])).unwrap(),
                format!("{hl}\n")
            );
            assert_eq!(
                text::to_string(&Chart::new(version, vec![lap()])).unwrap(),
                format!("{ch}\n")
            );
            assert_eq!(
                text::to_string(&WorldRecords::new(version, vec![lap()])).unwrap(),
                format!("{wr}\n")
            );
            // The same version choices reach every Serde format.
            let json = serde_json::to_value(Hotlaps::new(version, vec![lap()])).unwrap();
            assert_eq!(json[0].get("id_hl").is_some(), version >= V1_2);
            assert_eq!(json[0].get("split1").is_some(), version >= V1_3);
            assert_eq!(json[0].get("timestamp").is_some(), version >= V1_4);
            let time_key = if version >= V1_4 { "laptime" } else { "time" };
            assert_eq!(json[0][time_key], 30000);
            assert_eq!(json[0]["track"], "000");
        }
    }

    #[test]
    fn json_has_named_fields_numeric_units_and_no_version_tags() {
        let value = serde_json::to_value(WorldRecords::new(Version::V1_5, vec![lap()])).unwrap();
        assert_eq!(
            value,
            serde_json::json!([{
                "id_wr": 42, "track": "000", "car": "XFG",
                "split1": 10000, "split2": 20000, "split3": 0,
                "laptime": 30000, "flags_hlaps": 1025, "racername": "a racer",
                "timestamp": 1_000_000
            }])
        );
    }

    #[test]
    fn collections_are_complete_and_empty_output_is_unambiguous() {
        assert_eq!(
            text::to_string(&Hotlaps::new(Version::V1_5, vec![])).unwrap(),
            ""
        );
        assert_eq!(
            serde_json::to_string(&Chart::new(Version::V1_5, vec![])).unwrap(),
            "[]"
        );
        let rows = Hotlaps::new(Version::V1_0, (0..151).map(|_| lap()).collect());
        assert_eq!(text::to_string(&rows).unwrap().lines().count(), 151);
    }

    #[test]
    fn text_rejects_undefined_shapes_and_injected_rows() {
        #[derive(Serialize)]
        struct Optional {
            missing: Option<u32>,
        }
        assert!(text::to_string(&vec![Optional { missing: None }]).is_err());
        assert!(text::to_string(&vec![vec![1]]).is_err());
        assert!(text::to_string(&vec![1]).is_err());
        let mut invalid = lap();
        invalid.racername = "first\nsecond".into();
        assert!(text::to_string(&Chart::new(Version::V1_5, vec![invalid])).is_err());
    }

    #[test]
    fn text_propagates_writer_failures() {
        struct Broken;
        impl std::io::Write for Broken {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("broken"))
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        assert!(matches!(
            text::to_writer(Broken, &Hotlaps::new(Version::V1_0, vec![lap()])),
            Err(text::Error::Io(_))
        ));
    }

    #[test]
    fn typed_track_ids_round_trip_and_reject_unsupported_configurations() {
        let mappings = crate::track::TRACK_IDS;
        let mut ids = std::collections::HashSet::new();
        let mut tracks = std::collections::HashSet::new();
        for &(id, track) in mappings {
            assert!(ids.insert(id), "Duplicate Pubstat ID: {id}");
            assert!(tracks.insert(track), "Duplicate track: {track}");
            assert_eq!(track.to_pubstat_id(), Some(id));
            assert_eq!(Track::from_pubstat_id(id), Some(track));
        }
        for (legacy, canonical) in [
            (0, Track::Bl1),
            (1, Track::Bl1r),
            (10, Track::Bl2),
            (220, Track::Fe3),
            (410, Track::Ky2),
            (500, Track::We1),
            (661, Track::As7r),
            (7100, Track::Ro11),
        ] {
            assert_eq!(Track::from_pubstat_id(legacy), Some(canonical));
            assert_eq!(canonical.to_pubstat_id(), Some(legacy));
        }
        for invalid in [2, 999, 301, 1000, u16::MAX] {
            assert_eq!(Track::from_pubstat_id(invalid), None);
        }
        for invalid in [Track::Bl4, Track::Bl1x, Track::We3] {
            assert_eq!(invalid.to_pubstat_id(), None);
        }
    }

    #[test]
    fn captured_lfsworld_hotlaps_match_text_and_json() {
        let json = include_str!("../../fixtures/lfsworld/hl-1.5.json");
        let response = Hotlaps::new(Version::V1_5, captured_laps(json, "hl"));
        assert_eq!(
            text::to_string(&response).unwrap(),
            include_str!("../../fixtures/lfsworld/hl-1.5.txt")
        );
        assert_eq!(
            serde_json::to_value(response).unwrap(),
            serde_json::from_str::<serde_json::Value>(json).unwrap()
        );
    }

    #[test]
    fn captured_lfsworld_chart_matches_text_and_json() {
        let json = include_str!("../../fixtures/lfsworld/ch-1.5.json");
        let response = Chart::new(Version::V1_5, captured_laps(json, "ch"));
        assert_eq!(
            text::to_string(&response).unwrap(),
            include_str!("../../fixtures/lfsworld/ch-1.5.txt")
        );
        assert_eq!(
            serde_json::to_value(response).unwrap(),
            serde_json::from_str::<serde_json::Value>(json).unwrap()
        );
    }

    #[test]
    fn captured_lfsworld_world_records_match_text_and_json() {
        let json = include_str!("../../fixtures/lfsworld/wr-1.5.json");
        let response = WorldRecords::new(Version::V1_5, captured_laps(json, "wr"));
        assert_eq!(
            text::to_string(&response).unwrap(),
            include_str!("../../fixtures/lfsworld/wr-1.5.txt")
        );
        assert_eq!(
            serde_json::to_value(response).unwrap(),
            serde_json::from_str::<serde_json::Value>(json).unwrap()
        );
    }
}
