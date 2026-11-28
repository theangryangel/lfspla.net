//! Settings types that validate values during deserialization.

use std::{
    num::NonZeroU64,
    path::{Path, PathBuf},
    time::Duration,
};

use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};

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
    pub fn new(value: &str) -> Self {
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
    pub fn new(path: &str) -> Self {
        debug_assert!(!path.is_empty(), "default paths are non-empty");
        Self(PathBuf::from(path))
    }

    /// Returns the configured path, absolute once [`ConfigPath::resolve`] ran.
    pub fn path(&self) -> &Path {
        &self.0
    }

    /// Resolves a relative path against the configuration file's directory.
    pub fn resolve(&mut self, base_dir: &Path) {
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
