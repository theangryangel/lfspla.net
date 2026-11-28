//! Version-aware wr wire fields, shared by every output serializer.
use crate::{Lap, Version};
use serde::{Serialize, Serializer, ser::SerializeStruct};

#[derive(Debug)]
pub struct WorldRecords {
    version: Version,
    rows: Vec<Lap>,
}

impl crate::ser::xml::XmlRoot for WorldRecords {
    const ROOT: &'static str = "wr_data";
}

impl WorldRecords {
    pub fn new(version: Version, rows: Vec<Lap>) -> Self {
        Self { version, rows }
    }
}

impl Serialize for WorldRecords {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(self.rows.iter().map(|lap| Row {
            version: self.version,
            lap,
        }))
    }
}

/// One row. Version conditions select fields before the output format sees them.
struct Row<'a> {
    version: Version,
    lap: &'a Lap,
}
impl Serialize for Row<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let count = 5
            + usize::from(self.version >= Version::V1_2)
            + 3 * usize::from(self.version >= Version::V1_3)
            + usize::from(self.version >= Version::V1_5);
        let mut row = serializer.serialize_struct("WorldRecords", count)?;
        if self.version >= Version::V1_2 {
            row.serialize_field("id_wr", &self.lap.id)?;
        }
        row.serialize_field("track", &crate::track::PubstatTrack(self.lap.track))?;
        row.serialize_field("car", &self.lap.car)?;
        if self.version >= Version::V1_3 {
            row.serialize_field("split1", &self.lap.split1)?;
            row.serialize_field("split2", &self.lap.split2)?;
            row.serialize_field("split3", &self.lap.split3)?;
        }
        row.serialize_field(
            if self.version >= Version::V1_4 {
                "laptime"
            } else {
                "time"
            },
            &self.lap.laptime,
        )?;
        row.serialize_field("flags_hlaps", &self.lap.flags_hlaps)?;
        row.serialize_field("racername", &self.lap.racername)?;
        if self.version >= Version::V1_5 {
            row.serialize_field("timestamp", &self.lap.timestamp)?;
        }
        row.end()
    }
}
