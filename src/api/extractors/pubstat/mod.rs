//! Typed Pubstat query extraction and protocol capability validation.
mod compression;
mod format;
mod query;

use crate::api::{ApiState, pubstat::error::Error};
use axum::{
    extract::{FromRequestParts, Query},
    http::request::Parts,
};
pub(crate) use compression::Compression;
pub(crate) use format::Format;
use lfsplanet_pubstat::Version;
pub(crate) use query::{ChartQuery, HlQuery, WrQuery};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(tag = "action")]
pub(crate) enum Action {
    #[serde(rename = "hl")]
    Hotlaps(HlQuery),
    #[serde(rename = "ch")]
    Chart(ChartQuery),
    #[serde(rename = "wr")]
    WorldRecords(WrQuery),
    #[serde(
        rename = "pb",
        alias = "fuel",
        alias = "pst",
        alias = "hosts",
        alias = "teams",
        alias = "hl_log",
        alias = "counters"
    )]
    Unsupported,
    #[serde(other)]
    Unknown,
}

impl Action {
    pub(crate) fn rejection(&self) -> Error {
        if matches!(self, Self::Unknown) {
            Error::bad_request("Unknown action")
        } else {
            Error::unsupported("This action has no supported Pubstat data source")
        }
    }
}

#[derive(Deserialize)]
pub(crate) struct PubstatQuery {
    #[serde(default)]
    pub version: Version,
    #[serde(default, rename = "s")]
    pub format: Format,
    #[serde(flatten)]
    pub action: Action,
    #[serde(default, rename = "c")]
    pub compression: Compression,
}

impl FromRequestParts<ApiState> for PubstatQuery {
    type Rejection = Error;
    async fn from_request_parts(parts: &mut Parts, state: &ApiState) -> Result<Self, Error> {
        let Query(mut request) = Query::<Self>::from_request_parts(parts, state)
            .await
            .map_err(|error| Error::bad_request(error.body_text()))?;
        if request.format == Format::Php {
            return Err(Error::unsupported("PHP output is not implemented"));
        }
        if request.format != Format::Text && request.version < Version::V1_4 {
            return Err(Error::bad_request(
                "Alternate output formats require version 1.4 or higher",
            ));
        }
        match &mut request.action {
            Action::Hotlaps(query) => {
                if query.racer.trim().is_empty() {
                    return Err(Error::bad_request("racer is required"));
                }
            }
            Action::Chart(query) => {
                if let Some(country) = query.country.as_deref() {
                    if request.version < Version::V1_5 {
                        return Err(Error::bad_request("country requires version 1.5"));
                    }
                    let country = celes::Country::from_alpha2(country)
                        .map_err(|_| Error::bad_request("country must be an ISO alpha-2 code"))?;
                    query.country = Some(country.alpha2.to_owned());
                }
            }
            Action::WorldRecords(query) => {
                if request.version < Version::V1_4 && (query.track.is_some() || query.car.is_some())
                {
                    return Err(Error::bad_request(
                        "WR filters require version 1.4 or higher",
                    ));
                }
            }
            action => return Err(action.rejection()),
        }
        Ok(request)
    }
}
