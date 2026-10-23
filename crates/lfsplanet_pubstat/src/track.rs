//! Legacy three-digit track identifiers, independent of catalogue ordering.
//!
//! Documented S2 IDs and later IDs observed in captured exports. Codes refer
//! to configurations within the selected era (e.g. historical BL2 is rallycross).
use insim_core::track::Track;
use serde::{Serialize, Serializer, ser::Error};

/// Conversion between typed tracks and legacy Pubstat numeric identifiers.
pub trait TrackPubstatExt: Sized {
    fn to_pubstat_id(self) -> Option<u16>;
    fn from_pubstat_id(id: u16) -> Option<Self>;
}

impl TrackPubstatExt for Track {
    fn to_pubstat_id(self) -> Option<u16> {
        TRACK_IDS
            .iter()
            .find_map(|&(id, track)| (track == self).then_some(id))
    }

    fn from_pubstat_id(id: u16) -> Option<Self> {
        TRACK_IDS
            .iter()
            .find_map(|&(candidate, track)| (candidate == id).then_some(track))
    }
}

/// Wire adapter: preserve zero padding in text and JSON while storing a Track.
pub(crate) struct PubstatTrack(pub Track);

impl Serialize for PubstatTrack {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let id = self
            .0
            .to_pubstat_id()
            .ok_or_else(|| S::Error::custom("Track has no Pubstat identifier"))?;
        serializer.collect_str(&format_args!("{id:03}"))
    }
}

// Explicit mapping using the supplied documentation's location/config/reverse
// convention (000 = BL1, 001 = BL1R, 410 = KY2). Historical configuration
// identities belong to the selected era. Keep these IDs independent of enum
// declaration order and the current catalogue. Not every pair is independently
// documented. Later BL/WE/RO IDs were observed in the 1.5 capture fixtures;
// their canonical tracks follow that same configuration convention.
pub(crate) const TRACK_IDS: &[(u16, Track)] = &[
    (0, Track::Bl1),
    (1, Track::Bl1r),
    (10, Track::Bl2),
    (11, Track::Bl2r),
    (20, Track::Bl3),
    (21, Track::Bl3r),
    (100, Track::So1),
    (101, Track::So1r),
    (110, Track::So2),
    (111, Track::So2r),
    (120, Track::So3),
    (121, Track::So3r),
    (130, Track::So4),
    (131, Track::So4r),
    (140, Track::So5),
    (141, Track::So5r),
    (150, Track::So6),
    (151, Track::So6r),
    (200, Track::Fe1),
    (201, Track::Fe1r),
    (210, Track::Fe2),
    (211, Track::Fe2r),
    (220, Track::Fe3),
    (221, Track::Fe3r),
    (230, Track::Fe4),
    (231, Track::Fe4r),
    (240, Track::Fe5),
    (241, Track::Fe5r),
    (250, Track::Fe6),
    (251, Track::Fe6r),
    (300, Track::Au1),
    (310, Track::Au2),
    (320, Track::Au3),
    (330, Track::Au4),
    (400, Track::Ky1),
    (401, Track::Ky1r),
    (410, Track::Ky2),
    (411, Track::Ky2r),
    (420, Track::Ky3),
    (421, Track::Ky3r),
    (500, Track::We1),
    (501, Track::We1r),
    (510, Track::We2),
    (511, Track::We2r),
    (530, Track::We4),
    (531, Track::We4r),
    (540, Track::We5),
    (541, Track::We5r),
    (600, Track::As1),
    (601, Track::As1r),
    (610, Track::As2),
    (611, Track::As2r),
    (620, Track::As3),
    (621, Track::As3r),
    (630, Track::As4),
    (631, Track::As4r),
    (640, Track::As5),
    (641, Track::As5r),
    (650, Track::As6),
    (651, Track::As6r),
    (660, Track::As7),
    (661, Track::As7r),
    (700, Track::Ro1),
    (710, Track::Ro2),
    (720, Track::Ro3),
    (730, Track::Ro4),
    (740, Track::Ro5),
    (750, Track::Ro6),
    (760, Track::Ro7),
    (770, Track::Ro8),
    (780, Track::Ro9),
    (790, Track::Ro10),
    (7100, Track::Ro11),
];
