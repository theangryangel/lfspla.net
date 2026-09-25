//! Settings types that validate values during deserialization.

use std::{
    num::NonZeroU64,
    path::{Path, PathBuf},
    time::Duration,
};

use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};
use url::Url;

/// A duration in whole seconds, defaulting to `DEFAULT` and rejecting zero.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Seconds<const DEFAULT: u64>(Duration);

impl<const DEFAULT: u64> Seconds<DEFAULT> {
    pub const fn duration(self) -> Duration {
        self.0
    }
}

impl<const DEFAULT: u64> Default for Seconds<DEFAULT> {
    fn default() -> Self {
        Self(Duration::from_secs(DEFAULT))
    }
}

impl<const DEFAULT: u64> Serialize for Seconds<DEFAULT> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u64(self.0.as_secs())
    }
}

impl<'de, const DEFAULT: u64> Deserialize<'de> for Seconds<DEFAULT> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let seconds = NonZeroU64::deserialize(deserializer)?;
        Ok(Self(Duration::from_secs(seconds.get())))
    }
}

/// A non-empty string, trimmed for the emptiness check only.
///
/// Deliberately not `Debug`: OAuth client secrets are stored in one of these.
#[derive(Clone, Serialize)]
#[serde(transparent)]
pub struct NonEmptyString(String);

impl NonEmptyString {
    /// Wraps a known-good literal for use in a [`Default`] impl.
    pub(crate) fn new(value: &str) -> Self {
        debug_assert!(!value.trim().is_empty(), "default strings are non-empty");
        Self(value.to_owned())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for NonEmptyString {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        if value.trim().is_empty() {
            return Err(D::Error::custom("must not be empty"));
        }
        Ok(Self(value))
    }
}

/// A non-empty filesystem path from the configuration file.
///
/// [`ConfigPath::resolve`] makes relative paths start at the config directory.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct ConfigPath(PathBuf);

impl ConfigPath {
    /// Wraps a known-good literal for use in a [`Default`] impl.
    pub(crate) fn new(path: &str) -> Self {
        debug_assert!(!path.is_empty(), "default paths are non-empty");
        Self(PathBuf::from(path))
    }

    /// Returns the configured path, absolute once [`ConfigPath::resolve`] ran.
    pub fn path(&self) -> &Path {
        &self.0
    }

    /// Resolves a relative path against the configuration file's directory.
    pub(crate) fn resolve(&mut self, base_dir: &Path) {
        if self.0.is_relative() {
            self.0 = base_dir.join(&self.0);
        }
    }
}

impl<'de> Deserialize<'de> for ConfigPath {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let path = PathBuf::deserialize(deserializer)?;
        if path.as_os_str().is_empty() {
            return Err(D::Error::custom("must not be empty"));
        }
        Ok(Self(path))
    }
}

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

impl<'de> Deserialize<'de> for PublicBaseUrl {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let url = Url::deserialize(deserializer)?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err(D::Error::custom("must use http or https"));
        }
        if url.host_str().is_none() {
            return Err(D::Error::custom("must include a host"));
        }
        Ok(Self(url))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse<T: for<'de> Deserialize<'de>>(yaml: &str) -> anyhow::Result<T> {
        Ok(serde_saphyr::from_str(yaml)?)
    }

    #[test]
    fn seconds_default_comes_from_the_type_and_zero_is_rejected() {
        assert_eq!(Seconds::<30>::default().duration(), Duration::from_secs(30));
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

        let mut relative = parse::<ConfigPath>("data/storage").unwrap();
        relative.resolve(Path::new("/config"));
        assert_eq!(relative.path(), Path::new("/config/data/storage"));

        let mut absolute = parse::<ConfigPath>("/srv/lfs").unwrap();
        absolute.resolve(Path::new("/config"));
        assert_eq!(absolute.path(), Path::new("/srv/lfs"));
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
