//! LFS locations that group related track configurations.

use insim_core::track::Track;
use sea_orm::{DeriveActiveEnum, EnumIter};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// The LFS location a track configuration belongs to.
///
/// Locations without a dedicated category, such as Autocross, are `Other`.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize, ToSchema,
)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "UPPERCASE")]
pub enum TrackLocation {
    #[sea_orm(string_value = "BL")]
    Bl,
    #[sea_orm(string_value = "SO")]
    So,
    #[sea_orm(string_value = "FE")]
    Fe,
    #[sea_orm(string_value = "KY")]
    Ky,
    #[sea_orm(string_value = "AS")]
    As,
    #[sea_orm(string_value = "WE")]
    We,
    #[sea_orm(string_value = "RO")]
    Ro,
    #[sea_orm(string_value = "OTHER")]
    Other,
}

impl From<Track> for TrackLocation {
    fn from(track: Track) -> Self {
        match &track.code()[..2] {
            "BL" => Self::Bl,
            "SO" => Self::So,
            "FE" => Self::Fe,
            "KY" => Self::Ky,
            "AS" => Self::As,
            "WE" => Self::We,
            "RO" => Self::Ro,
            _ => Self::Other,
        }
    }
}

impl TrackLocation {
    /// Human-readable name for this LFS location.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Bl => "Blackwood",
            Self::So => "South City",
            Self::Fe => "Fern Bay",
            Self::Ky => "Kyoto Ring",
            Self::As => "Aston",
            Self::We => "Westhill",
            Self::Ro => "Rockingham",
            Self::Other => "Other",
        }
    }
}

#[cfg(test)]
mod tests {
    use insim_core::track::Track;

    use super::TrackLocation;

    #[test]
    fn derives_a_location_from_a_track_code() {
        assert_eq!(TrackLocation::from(Track::Bl1), TrackLocation::Bl);
        assert_eq!(TrackLocation::from(Track::So4r), TrackLocation::So);
        assert_eq!(TrackLocation::from(Track::Fe1), TrackLocation::Fe);
        assert_eq!(TrackLocation::from(Track::Ky1), TrackLocation::Ky);
        assert_eq!(TrackLocation::from(Track::As1), TrackLocation::As);
        assert_eq!(TrackLocation::from(Track::We1), TrackLocation::We);
        assert_eq!(TrackLocation::from(Track::Ro1), TrackLocation::Ro);
        assert_eq!(TrackLocation::from(Track::Au1), TrackLocation::Other);
    }

    #[test]
    fn serializes_using_uppercase_location_codes() {
        assert_eq!(serde_json::to_value(TrackLocation::Bl).unwrap(), "BL");
        assert_eq!(serde_json::to_value(TrackLocation::Other).unwrap(), "OTHER");
    }
}
