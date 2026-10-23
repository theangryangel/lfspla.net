//! Version-aware hl wire fields, shared by every output serializer.
use crate::{Lap, Version};
use serde::{Serialize, Serializer, ser::SerializeStruct};

#[derive(Debug)]
pub struct Hotlaps {
    version: Version,
    rows: Vec<Lap>,
}

impl crate::ser::xml::XmlRoot for Hotlaps {
    const ROOT: &'static str = "hl_data";
}

impl Hotlaps {
    pub fn new(version: Version, rows: Vec<Lap>) -> Self {
        Self { version, rows }
    }
}

impl Serialize for Hotlaps {
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
        let count = 4
            + usize::from(self.version >= Version::V1_2)
            + 3 * usize::from(self.version >= Version::V1_3)
            + usize::from(self.version >= Version::V1_4);
        let mut row = serializer.serialize_struct("Hotlaps", count)?;
        if self.version >= Version::V1_2 {
            row.serialize_field("id_hl", &self.lap.id)?;
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
        if self.version >= Version::V1_4 {
            row.serialize_field("timestamp", &self.lap.timestamp)?;
        }
        row.end()
    }
}
