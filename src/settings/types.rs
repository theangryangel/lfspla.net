//! Application-specific configuration values and shared primitive imports.

pub use lfsplanet_core::settings::Seconds;
#[cfg(test)]
use lfsplanet_core::settings::{ConfigPath, NonEmptyString};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};
#[cfg(test)]
use std::time::Duration;
use url::Url;

/// The encryption key for the private session cookie.
///
/// Deliberately not `Debug`: this is the key that signs every session.
#[derive(Clone)]
pub struct SessionKey([u8; SessionKey::BYTES]);

impl SessionKey {
    /// Byte length required by the private cookie jar.
    const BYTES: usize = 64;

    /// Returns the raw key material.
    pub fn as_slice(&self) -> &[u8] {
        &self.0
    }
}

impl Default for SessionKey {
    fn default() -> Self {
        Self(rand::random())
    }
}

impl Serialize for SessionKey {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&hex::encode(self.0))
    }
}

impl<'de> Deserialize<'de> for SessionKey {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        let bytes = hex::decode(&text)
            .map_err(|_| D::Error::custom("must contain hexadecimal characters"))?;
        let bytes: [u8; Self::BYTES] = bytes.try_into().map_err(|_| {
            D::Error::custom(format!(
                "must encode exactly {} bytes ({} hexadecimal characters)",
                Self::BYTES,
                Self::BYTES * 2
            ))
        })?;
        Ok(Self(bytes))
    }
}

/// The public base URL used to construct absolute links back to this service.
#[derive(Clone, Debug, Serialize)]
#[serde(transparent)]
pub struct PublicBaseUrl(Url);

impl PublicBaseUrl {
    pub fn url(&self) -> &Url {
        &self.0
    }
}

impl TryFrom<Url> for PublicBaseUrl {
    type Error = anyhow::Error;

    fn try_from(url: Url) -> Result<Self, Self::Error> {
        anyhow::ensure!(
            matches!(url.scheme(), "http" | "https"),
            "must use http or https"
        );
        anyhow::ensure!(url.host_str().is_some(), "must include a host");
        Ok(Self(url))
    }
}

impl<'de> Deserialize<'de> for PublicBaseUrl {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let url = Url::deserialize(deserializer)?;
        Self::try_from(url).map_err(D::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse<T: for<'de> Deserialize<'de>>(yaml: &str) -> anyhow::Result<T> {
        Ok(serde_saphyr::from_str(yaml)?)
    }

    #[test]
    fn seconds_parse_and_zero_is_rejected() {
        assert_eq!(
            parse::<Seconds<30>>("5").unwrap().duration(),
            Duration::from_secs(5)
        );
        assert!(parse::<Seconds<30>>("0").is_err());
        assert!(parse::<Seconds<30>>("-1").is_err());
    }

    #[test]
    fn non_empty_string_rejects_blank_values() {
        assert_eq!(parse::<NonEmptyString>("value").unwrap().as_str(), "value");
        assert!(parse::<NonEmptyString>(r#""""#).is_err());
        assert!(parse::<NonEmptyString>(r#""   ""#).is_err());
    }

    #[test]
    fn config_path_rejects_empty_and_resolves_relative_paths() {
        assert!(parse::<ConfigPath>(r#""""#).is_err());

        let root = tempfile::tempdir().unwrap();
        let base = root.path().join("config");
        let mut relative = parse::<ConfigPath>("data/storage").unwrap();
        relative.resolve(&base);
        assert_eq!(relative.path(), base.join("data/storage"));

        let path = root.path().join("lfs");
        let yaml = serde_saphyr::to_string(&path).unwrap();
        let mut absolute = parse::<ConfigPath>(&yaml).unwrap();
        absolute.resolve(&base);
        assert_eq!(absolute.path(), path);
    }

    #[test]
    fn session_key_must_be_64_hex_encoded_bytes() {
        let key = parse::<SessionKey>(&format!(r#""{}""#, "a5".repeat(64))).unwrap();
        assert_eq!(key.as_slice().len(), 64);

        assert!(parse::<SessionKey>(&format!(r#""{}""#, "a5".repeat(63))).is_err());
        assert!(parse::<SessionKey>(&format!(r#""{}""#, "zz".repeat(64))).is_err());
    }

    #[test]
    fn public_base_url_requires_an_http_scheme_and_a_host() {
        assert!(parse::<PublicBaseUrl>("http://localhost:8000").is_ok());
        assert!(parse::<PublicBaseUrl>("https://lfspla.net").is_ok());
        assert!(parse::<PublicBaseUrl>("ftp://lfspla.net").is_err());
        assert!(parse::<PublicBaseUrl>("file:///tmp").is_err());
    }
}
