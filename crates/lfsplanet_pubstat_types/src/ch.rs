//! Version-aware ch wire fields, shared by every output serializer.
use crate::{Lap, Version};
use serde::{Serialize, Serializer, ser::SerializeStruct};

#[derive(Debug)]
pub struct Chart {
    version: Version,
    rows: Vec<Lap>,
}

impl crate::ser::xml::XmlRoot for Chart {
    const ROOT: &'static str = "ch_data";
}

impl Chart {
    pub fn new(version: Version, rows: Vec<Lap>) -> Self {
        Self { version, rows }
    }
}

impl Serialize for Chart {
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
        let count = 3
            + usize::from(self.version >= Version::V1_4)
            + 3 * usize::from(self.version >= Version::V1_3);
        let mut row = serializer.serialize_struct("Chart", count)?;
        if self.version >= Version::V1_4 {
            row.serialize_field("id_hl", &self.lap.id)?;
        }
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
        row.end()
    }
}
