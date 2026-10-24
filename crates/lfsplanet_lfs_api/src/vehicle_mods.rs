//! The official LFS Vehicle Mods catalogue.

use serde::{Deserialize, Serialize};
use url::Url;

use crate::{Error, LfsClient, endpoints};

/// One current mod record returned by the official catalogue.
#[derive(Debug, Clone)]
pub struct RemoteVehicleMod {
    pub id: String,
    pub name: String,
    pub version: u16,
    pub class: Option<u8>,
    pub author_username: Option<String>,
    pub work_in_progress: bool,
    pub is_private: bool,
    pub is_tweak_mod: bool,
    pub published_at: Option<i64>,
    pub cover_thumb_url: Option<String>,
    pub metadata: serde_json::Value,
}

/// A validated cover image downloaded from the LFS CDN.
#[derive(Debug, Clone)]
pub struct CoverImage {
    pub bytes: bytes::Bytes,
    pub content_type: String,
}

impl LfsClient {
    /// Fetches the full response, including structured vehicle specifications.
    pub async fn vehicle_mod(&self, id: &str) -> Result<RemoteVehicleMod, Error> {
        if id.len() != 6 || !id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(Error::InvalidCatalogue("invalid mod ID".to_owned()));
        }
        let token = self.service_token().await?;
        let response = self
            .http
            .get(format!("{}/{id}", endpoints::VEHICLE_MODS))
            .bearer_auth(token)
            .send()
            .await?
            .error_for_status()?
            .json::<DetailEnvelope>()
            .await?;
        parse_detail(id, response.data)
    }

    /// Lists every current mod in the official catalogue.
    pub async fn vehicle_mods(&self) -> Result<Vec<RemoteVehicleMod>, Error> {
        let access_token = self.service_token().await?;
        let response = self
            .http
            .get(endpoints::VEHICLE_MODS)
            .bearer_auth(access_token)
            .send()
            .await?
            .error_for_status()?
            .json::<CatalogueEnvelope>()
            .await?;
        parse_catalogue(response.data)
    }

    /// Downloads one prevalidated Vehicle Mods cover thumbnail.
    pub async fn vehicle_mod_cover(&self, thumbnail_url: &str) -> Result<CoverImage, Error> {
        let url = lfs_cover_url(thumbnail_url)?;
        let response = self.http.get(url).send().await?.error_for_status()?;
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.split(';').next())
            .map(str::trim)
            .filter(|value| {
                matches!(
                    *value,
                    "image/avif" | "image/jpeg" | "image/png" | "image/webp"
                )
            })
            .ok_or_else(|| {
                Error::InvalidCatalogue("cover has an unsupported content type".to_owned())
            })?
            .to_owned();
        let bytes = response.bytes().await?;
        if bytes.len() > 10 * 1024 * 1024 {
            return Err(Error::InvalidCatalogue("cover exceeds 10 MiB".to_owned()));
        }
        Ok(CoverImage {
            bytes,
            content_type,
        })
    }
}

#[derive(Deserialize)]
struct CatalogueEnvelope {
    data: Vec<serde_json::Value>,
}

#[derive(Deserialize)]
struct DetailEnvelope {
    data: serde_json::Value,
}

/// Shared schema for list and detail responses. Required fields have no defaults:
/// missing or incorrectly typed data must not become a successful cache marker.
/// Fields we do not consume are retained without imposing a schema on them.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "mirrors independent flags in the upstream API"
)]
pub struct VehicleModMetadata {
    pub id: String,
    pub name: String,
    pub version: u16,
    pub class: Option<u8>,
    #[serde(rename = "userName")]
    pub user_name: Option<String>,
    pub wip: bool,
    #[serde(rename = "private")]
    pub is_private: bool,
    #[serde(rename = "tweakMod")]
    pub is_tweak_mod: bool,
    #[serde(rename = "publishedAt")]
    pub published_at: Option<i64>,
    #[serde(rename = "coverUrl")]
    pub cover_url: Option<String>,
    pub approved: bool,
    #[serde(rename = "ev")]
    pub electric: bool,
    pub rating: f64,
    #[serde(rename = "numRatings")]
    pub num_ratings: u64,
    #[serde(rename = "numDownloads")]
    pub num_downloads: u64,
    /// Absent in catalogue lists; required by the individual detail endpoint.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vehicle: Option<VehicleModSpecifications>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

/// Numeric fields used by the explorer, in the units supplied by LFS.
/// Zero is a valid value (for example engine displacement on an electric vehicle).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VehicleModSpecifications {
    pub ice_cc: f64,
    pub ice_num_cylinders: u8,
    pub ice_layout: u8,
    pub drive: u8,
    pub shift_type: u8,
    pub power: f64,
    pub max_power_rpm: u32,
    pub torque: f64,
    pub max_torque_rpm: u32,
    pub mass: f64,
    pub power_weight_ratio: f64,
    pub fuel_tank_size: f64,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl VehicleModMetadata {
    /// Derives the smaller cover representation without letting one malformed
    /// optional URL invalidate the complete Vehicle Mods catalogue.
    fn cover_thumb_url(&self) -> Option<String> {
        let cover_url = self.cover_url.as_deref()?.trim();
        if cover_url.is_empty() {
            return None;
        }
        let mut url = lfs_cover_url(cover_url).ok()?;
        url.path_segments_mut().ok()?.push("thumb");
        Some(url.into())
    }
}

/// Highest vehicle class the official catalogue assigns.
const MAX_VEHICLE_CLASS: u8 = 14;
/// Mod versions are the three-digit revision LFS writes into replay metadata.
const MAX_MOD_VERSION: u16 = 999;

fn parse_catalogue(entries: Vec<serde_json::Value>) -> Result<Vec<RemoteVehicleMod>, Error> {
    if entries.is_empty() {
        return Err(Error::InvalidCatalogue(
            "catalogue is unexpectedly empty".to_owned(),
        ));
    }
    let mods = entries
        .into_iter()
        .map(|metadata| {
            let entry: VehicleModMetadata = serde_json::from_value(metadata.clone())
                .map_err(|error| Error::InvalidCatalogue(error.to_string()))?;
            let cover_thumb_url = entry.cover_thumb_url();

            let id = entry.id.to_ascii_uppercase();
            if id.len() != 6 || !id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                return Err(Error::InvalidCatalogue(format!("invalid mod id {id:?}")));
            }
            let name = entry.name.trim();
            if name.is_empty() {
                return Err(Error::InvalidCatalogue("missing name".to_owned()));
            }
            if entry.version > MAX_MOD_VERSION {
                return Err(Error::InvalidCatalogue(format!(
                    "invalid mod version {}",
                    entry.version
                )));
            }
            if entry.class.is_some_and(|class| class > MAX_VEHICLE_CLASS) {
                return Err(Error::InvalidCatalogue("invalid vehicle class".to_owned()));
            }

            Ok(RemoteVehicleMod {
                id,
                name: name.to_owned(),
                version: entry.version,
                class: entry.class,
                author_username: entry.user_name,
                work_in_progress: entry.wip,
                is_private: entry.is_private,
                is_tweak_mod: entry.is_tweak_mod,
                published_at: entry.published_at,
                cover_thumb_url,
                metadata,
            })
        })
        .collect::<Result<Vec<_>, Error>>()?;
    // WIP, private, and tweak mods are not part of lfsplanet's public or
    // replay-admission catalogue. Filter here so no later synchronization
    // step can use them.
    Ok(mods
        .into_iter()
        .filter(|modification| {
            !modification.work_in_progress && !modification.is_private && !modification.is_tweak_mod
        })
        .collect())
}

fn parse_detail(id: &str, metadata: serde_json::Value) -> Result<RemoteVehicleMod, Error> {
    let typed: VehicleModMetadata = serde_json::from_value(metadata.clone())
        .map_err(|error| Error::InvalidCatalogue(error.to_string()))?;
    if typed.vehicle.is_none() {
        return Err(Error::InvalidCatalogue(
            "missing vehicle specifications".to_owned(),
        ));
    }
    parse_catalogue(vec![metadata])?
        .pop()
        .filter(|entry| entry.id.eq_ignore_ascii_case(id))
        .ok_or_else(|| Error::InvalidCatalogue("mod is unavailable or its ID changed".to_owned()))
}

fn lfs_cover_url(value: &str) -> Result<Url, Error> {
    let url = Url::parse(value)
        .map_err(|error| Error::InvalidCatalogue(format!("invalid cover URL: {error}")))?;
    let trusted_host = url.host_str().is_some_and(|host| {
        host.eq_ignore_ascii_case("lfs.net") || host.to_ascii_lowercase().ends_with(".lfs.net")
    });
    if url.scheme() != "https" || !trusted_host {
        return Err(Error::InvalidCatalogue(
            "cover URL is not hosted by LFS".to_owned(),
        ));
    }
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metadata() -> serde_json::Value {
        serde_json::json!({
            "id": "41a2a0", "name": "PIRAN FIREFLY 200", "version": 7,
            "class": 2, "userName": "PiranMOTO", "wip": false,
            "private": false, "tweakMod": false, "approved": true, "ev": false,
            "rating": 4.84, "numRatings": 28, "numDownloads": 100,
            "publishedAt": 1_700_000_000, "coverUrl": "https://www.lfs.net/cover.png"
        })
    }

    fn specifications() -> serde_json::Value {
        serde_json::json!({
            "iceCc": 1998.0, "iceNumCylinders": 6, "iceLayout": 0,
            "drive": 1, "shiftType": 1, "power": 160.0, "maxPowerRpm": 6416,
            "torque": 275.0, "maxTorqueRpm": 4512, "mass": 1425.0,
            "powerWeightRatio": 112.0, "fuelTankSize": 65.0
        })
    }

    #[test]
    fn parses_the_documented_catalogue_envelope_fields() {
        let parsed = parse_catalogue(vec![metadata()]).unwrap();
        assert_eq!(parsed[0].id, "41A2A0");
        assert_eq!(parsed[0].name, "PIRAN FIREFLY 200");
        assert_eq!(parsed[0].version, 7);
        assert_eq!(
            parsed[0].cover_thumb_url.as_deref(),
            Some("https://www.lfs.net/cover.png/thumb")
        );
        let typed: VehicleModMetadata = serde_json::from_value(parsed[0].metadata.clone()).unwrap();
        assert!(typed.vehicle.is_none());
    }

    #[test]
    fn rejects_an_empty_or_structurally_invalid_full_refresh() {
        assert!(parse_catalogue(Vec::new()).is_err());
        let mut invalid = metadata();
        invalid["id"] = "not-an-id".into();
        assert!(parse_catalogue(vec![invalid]).is_err());
    }

    #[test]
    fn requires_consumed_metadata_fields_and_their_types() {
        for key in [
            "id",
            "name",
            "version",
            "wip",
            "private",
            "tweakMod",
            "approved",
            "ev",
            "rating",
            "numRatings",
            "numDownloads",
        ] {
            let mut missing = metadata();
            missing.as_object_mut().unwrap().remove(key);
            assert!(parse_catalogue(vec![missing]).is_err(), "missing {key}");
        }
        for (key, value) in [
            ("numRatings", serde_json::json!(-1)),
            ("rating", serde_json::json!("five")),
            ("approved", serde_json::Value::Null),
        ] {
            let mut invalid = metadata();
            invalid[key] = value;
            assert!(parse_catalogue(vec![invalid]).is_err(), "invalid {key}");
        }
    }

    #[test]
    fn detail_requires_complete_numeric_specifications() {
        assert!(parse_detail("41A2A0", metadata()).is_err());
        let mut detail = metadata();
        detail["vehicle"] = serde_json::json!({});
        assert!(parse_detail("41A2A0", detail.clone()).is_err());
        for key in specifications().as_object().unwrap().keys() {
            detail["vehicle"] = specifications();
            detail["vehicle"].as_object_mut().unwrap().remove(key);
            assert!(
                parse_detail("41A2A0", detail.clone()).is_err(),
                "missing {key}"
            );
        }
        detail["vehicle"] = specifications();
        detail["vehicle"]["power"] = "160 kW".into();
        assert!(parse_detail("41A2A0", detail.clone()).is_err());
        detail["vehicle"] = specifications();
        detail["vehicle"]["iceCc"] = 0.into();
        assert!(parse_detail("41A2A0", detail.clone()).is_ok());
        assert!(parse_detail("ABCDEF", detail).is_err());
    }

    #[test]
    fn retains_extra_fields_at_both_levels() {
        let mut detail = metadata();
        detail["vehicle"] = specifications();
        detail["futureField"] = serde_json::json!({"nested": [1, "two", null]});
        detail["vehicle"]["futureSpec"] =
            serde_json::json!({"unit": "something", "values": [3, 4]});
        let parsed = parse_detail("41A2A0", detail.clone()).unwrap();
        assert_eq!(parsed.metadata, detail);
        let typed: VehicleModMetadata = serde_json::from_value(detail.clone()).unwrap();
        assert_eq!(typed.extra["futureField"], detail["futureField"]);
        assert_eq!(
            typed.vehicle.as_ref().unwrap().extra["futureSpec"],
            detail["vehicle"]["futureSpec"]
        );
        assert_eq!(serde_json::to_value(typed).unwrap(), detail);
    }

    #[test]
    fn omits_work_in_progress_private_and_tweak_mods() {
        for key in ["wip", "private", "tweakMod"] {
            let mut hidden = metadata();
            hidden[key] = true.into();
            assert!(parse_catalogue(vec![hidden.clone()]).unwrap().is_empty());
            hidden["vehicle"] = specifications();
            assert!(parse_detail("41A2A0", hidden).is_err());
        }
    }

    #[test]
    fn derives_thumbnails_only_from_https_lfs_cover_urls() {
        let entry = |cover_url| {
            let mut entry: VehicleModMetadata = serde_json::from_value(metadata()).unwrap();
            entry.cover_url = cover_url;
            entry
        };
        assert_eq!(
            entry(Some("https://www.lfs.net/attachment/788868".to_owned()))
                .cover_thumb_url()
                .as_deref(),
            Some("https://www.lfs.net/attachment/788868/thumb")
        );
        assert!(
            entry(Some("https://cdn.lfs.net/cover.png".to_owned()))
                .cover_thumb_url()
                .is_some()
        );
        assert!(
            entry(Some("http://www.lfs.net/cover.png".to_owned()))
                .cover_thumb_url()
                .is_none()
        );
        assert!(
            entry(Some("https://lfs.net.example/cover.png".to_owned()))
                .cover_thumb_url()
                .is_none()
        );
    }
}
