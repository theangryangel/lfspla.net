//! XML framing around the same version-aware rows used by text and JSON.
use serde::{Serialize, Serializer, ser::SerializeStruct};
use std::io::Write;

/// The action-specific XML root name. Rows are emitted as repeated entries.
pub trait XmlRoot: Serialize {
    const ROOT: &'static str;
}

struct Envelope<'a, T>(&'a T);

impl<T: XmlRoot> Serialize for Envelope<'_, T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut root = serializer.serialize_struct(T::ROOT, 1)?;
        root.serialize_field("entry", self.0)?;
        root.end()
    }
}

/// Emit a UTF-8 document with an encoding declaration matching its bytes.
pub fn to_string<T: XmlRoot>(value: &T) -> Result<String, serde_xml_rs::Error> {
    serde_xml_rs::to_string(&Envelope(value))
}

pub fn to_writer<W: Write, T: XmlRoot>(writer: W, value: &T) -> Result<(), serde_xml_rs::Error> {
    serde_xml_rs::to_writer(writer, &Envelope(value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Version, ch::Chart, hl::Hotlaps, ser::test_support::captured_laps, wr::WorldRecords,
    };
    use ::xml::reader::{EventReader, XmlEvent};

    fn events(document: &str) -> Vec<XmlEvent> {
        EventReader::from_str(document)
            .into_iter()
            .filter_map(|event| match event.unwrap() {
                XmlEvent::StartDocument { .. }
                | XmlEvent::EndDocument
                | XmlEvent::Whitespace(_) => None,
                event => Some(event),
            })
            .collect()
    }

    fn assert_capture<T: XmlRoot>(response: &T, captured: &str) {
        // Captures are ASCII with conflicting HTTP/XML encoding metadata.
        // Compare XML structure and values with a truthful UTF-8 declaration.
        assert!(captured.is_ascii());
        let captured = captured.replacen("windows-1252", "UTF-8", 1);
        let actual = to_string(response).unwrap();
        assert!(actual.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
        assert_eq!(events(&actual), events(&captured));
    }

    #[test]
    fn captured_action_envelopes_and_fields_match() {
        assert_capture(
            &Hotlaps::new(
                Version::V1_5,
                captured_laps(include_str!("../../fixtures/lfsworld/hl-1.5.json"), "hl"),
            ),
            include_str!("../../fixtures/lfsworld/hl-1.5.xml"),
        );
        assert_capture(
            &Chart::new(
                Version::V1_5,
                captured_laps(include_str!("../../fixtures/lfsworld/ch-1.5.json"), "ch"),
            ),
            include_str!("../../fixtures/lfsworld/ch-1.5.xml"),
        );
        assert_capture(
            &WorldRecords::new(
                Version::V1_5,
                captured_laps(include_str!("../../fixtures/lfsworld/wr-1.5.json"), "wr"),
            ),
            include_str!("../../fixtures/lfsworld/wr-1.5.xml"),
        );
    }

    #[test]
    fn empty_collections_keep_their_action_root_without_entries() {
        assert_eq!(
            events(&to_string(&Hotlaps::new(Version::V1_5, vec![])).unwrap()),
            events("<hl_data/>")
        );
        assert_eq!(
            events(&to_string(&Chart::new(Version::V1_5, vec![])).unwrap()),
            events("<ch_data/>")
        );
        assert_eq!(
            events(&to_string(&WorldRecords::new(Version::V1_5, vec![])).unwrap()),
            events("<wr_data/>")
        );
    }

    #[test]
    fn utf8_and_xml_escaping_preserve_names_and_version_fields() {
        let mut laps = captured_laps(include_str!("../../fixtures/lfsworld/wr-1.5.json"), "wr");
        let name = "Åsa & <driver> \"quotes\"";
        laps[0].racername = name.into();
        let xml = to_string(&WorldRecords::new(Version::V1_4, laps)).unwrap();
        assert!(xml.contains("&amp;"));
        assert!(xml.contains("&lt;"));
        assert!(!xml.contains("<timestamp>"));
        assert!(events(&xml).contains(&XmlEvent::Characters(name.into())));
    }
}
