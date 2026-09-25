//! The official LFS Vehicle Mods catalogue.

use serde::Deserialize;
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

/// One entry of the catalogue envelope, as the official API documents it.
///
/// The raw `Value` is kept alongside this because the whole entry is stored,
/// but every field the application reads is declared here rather than walked
/// out of the `Value` by hand.
#[derive(Deserialize)]
struct CatalogueEntry {
    id: String,
    name: String,
    version: u16,
    class: Option<u8>,
    #[serde(rename = "userName")]
    user_name: Option<String>,
    #[serde(default)]
    wip: bool,
    #[serde(rename = "private", default)]
    is_private: bool,
    #[serde(rename = "tweakMod", default)]
    is_tweak_mod: bool,
    #[serde(rename = "publishedAt")]
    published_at: Option<i64>,
    #[serde(rename = "coverUrl")]
    cover_url: Option<String>,
}

impl CatalogueEntry {
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
            let entry: CatalogueEntry = serde_json::from_value(metadata.clone())
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

    #[test]
    fn parses_the_documented_catalogue_envelope_fields() {
        let entries = serde_json::json!([{
            "id": "41a2a0",
            "name": "PIRAN FIREFLY 200",
            "version": 7,
            "class": 2,
            "userName": "PiranMOTO",
            "wip": false,
            "private": false,
            "tweakMod": false,
            "publishedAt": 1_700_000_000,
            "coverUrl": "https://www.lfs.net/cover.png"
        }]);
        let parsed = parse_catalogue(entries.as_array().unwrap().clone()).unwrap();
        assert_eq!(parsed[0].id, "41A2A0");
        assert_eq!(parsed[0].name, "PIRAN FIREFLY 200");
        assert_eq!(parsed[0].version, 7);
        assert_eq!(
            parsed[0].cover_thumb_url.as_deref(),
            Some("https://www.lfs.net/cover.png/thumb")
        );
    }

    #[test]
    fn rejects_an_empty_or_structurally_invalid_full_refresh() {
        assert!(parse_catalogue(Vec::new()).is_err());
        assert!(
            parse_catalogue(vec![serde_json::json!({
                "id": "not-an-id",
                "name": "Example",
                "version": 1
            })])
            .is_err()
        );
    }

    #[test]
    fn omits_work_in_progress_private_and_tweak_mods() {
        let parsed = parse_catalogue(vec![serde_json::json!({
            "id": "41a2a0",
            "name": "Example",
            "version": 1,
            "wip": true
        })])
        .unwrap();
        assert!(parsed.is_empty());

        let parsed = parse_catalogue(vec![serde_json::json!({
            "id": "41a2a0",
            "name": "Example",
            "version": 1,
            "tweakMod": true
        })])
        .unwrap();
        assert!(parsed.is_empty());

        let parsed = parse_catalogue(vec![serde_json::json!({
            "id": "41a2a0",
            "name": "Example",
            "version": 1,
            "private": true
        })])
        .unwrap();
        assert!(parsed.is_empty());
    }

    #[test]
    fn derives_thumbnails_only_from_https_lfs_cover_urls() {
        let entry = |cover_url| CatalogueEntry {
            id: "41a2a0".to_owned(),
            name: "Example".to_owned(),
            version: 1,
            class: None,
            user_name: None,
            wip: false,
            is_private: false,
            is_tweak_mod: false,
            published_at: None,
            cover_url,
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
