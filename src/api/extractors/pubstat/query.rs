//! Typed action queries. Serde owns scalar parsing; only Pubstat track IDs
//! and abbreviated controller names need protocol-specific adaptation.
use crate::models::hotlap::SteeringInput;
use insim_core::{track::Track, vehicle::Vehicle};
use lfsplanet_pubstat::track::TrackPubstatExt;
use serde::{Deserialize, Deserializer, de::Error};

#[derive(Deserialize)]
pub(crate) struct HlQuery {
    pub racer: String,
}

#[derive(Deserialize)]
pub(crate) struct ChartQuery {
    #[serde(deserialize_with = "deserialize_track")]
    pub track: Track,
    pub car: Vehicle,
    pub control: Option<Control>,
    pub country: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct WrQuery {
    #[serde(default, deserialize_with = "deserialize_optional_track")]
    pub track: Option<Track>,
    pub car: Option<Vehicle>,
}

fn deserialize_track<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Track, D::Error> {
    // Flattened, internally tagged queries buffer URL values as strings.
    // Delegate numeric parsing to u16 before looking up the protocol identity.
    let id = String::deserialize(deserializer)?
        .parse()
        .map_err(D::Error::custom)?;
    Track::from_pubstat_id(id).ok_or_else(|| D::Error::custom("Unsupported Pubstat track ID"))
}

fn deserialize_optional_track<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Track>, D::Error> {
    Option::<String>::deserialize(deserializer)?
        .map(|value| {
            let id = value.parse().map_err(D::Error::custom)?;
            Track::from_pubstat_id(id)
                .ok_or_else(|| D::Error::custom("Unsupported Pubstat track ID"))
        })
        .transpose()
}

#[derive(Clone, Copy, Deserialize)]
pub(crate) enum Control {
    #[serde(rename = "w")]
    Wheel,
    #[serde(rename = "m")]
    Mouse,
    #[serde(rename = "ks")]
    KeyboardStabilised,
    #[serde(rename = "kn")]
    Keyboard,
}

impl From<Control> for SteeringInput {
    fn from(control: Control) -> Self {
        match control {
            Control::Wheel => Self::Wheel,
            Control::Mouse => Self::Mouse,
            Control::KeyboardStabilised => Self::KeyboardStabilised,
            Control::Keyboard => Self::Keyboard,
        }
    }
}
