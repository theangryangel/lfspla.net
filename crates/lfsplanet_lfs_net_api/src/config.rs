//! Configuration for reaching lfs.net: API credentials and request limits.

use std::{num::NonZeroU64, time::Duration};

use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};

use crate::{LfsClient, http_client};

const DEFAULT_TIMEOUT_SECONDS: u64 = 15;

/// How the application reaches lfs.net.
#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// OAuth client credentials, when the integration is configured.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oauth: Option<OAuthCredentials>,
    /// Maximum time allowed for one outbound lfs.net HTTP request.
    #[serde(rename = "outbound_http_timeout_seconds")]
    pub timeout: Timeout,
}

impl Config {
    /// The bound for one outbound lfs.net request.
    #[must_use]
    pub fn timeout(&self) -> Duration {
        self.timeout.0
    }

    /// Creates an API client when OAuth credentials are configured.
    pub fn client(&self) -> reqwest::Result<Option<LfsClient>> {
        let Some(oauth) = &self.oauth else {
            return Ok(None);
        };
        Ok(Some(LfsClient::new(
            http_client(self.timeout())?,
            oauth.client_id.0.clone(),
            oauth.client_secret.0.clone(),
        )))
    }
}

/// OAuth client credentials issued by LFS. Both values must be non-blank.
///
/// Deliberately not `Debug`: it holds the client secret.
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OAuthCredentials {
    client_id: NonBlank,
    client_secret: NonBlank,
}

impl OAuthCredentials {
    /// Wraps known-good values, such as placeholders in a generated config.
    #[must_use]
    pub fn new(client_id: &str, client_secret: &str) -> Self {
        debug_assert!(
            !client_id.trim().is_empty() && !client_secret.trim().is_empty(),
            "credentials are non-blank"
        );
        Self {
            client_id: NonBlank(client_id.to_owned()),
            client_secret: NonBlank(client_secret.to_owned()),
        }
    }

    #[must_use]
    pub fn client_id(&self) -> &str {
        &self.client_id.0
    }

    #[must_use]
    pub fn client_secret(&self) -> &str {
        &self.client_secret.0
    }
}

/// A request bound in whole seconds, defaulting to 15 and rejecting zero.
#[derive(Clone, Copy)]
pub struct Timeout(Duration);

impl Default for Timeout {
    fn default() -> Self {
        Self(Duration::from_secs(DEFAULT_TIMEOUT_SECONDS))
    }
}

impl Serialize for Timeout {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u64(self.0.as_secs())
    }
}

impl<'de> Deserialize<'de> for Timeout {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let seconds = NonZeroU64::deserialize(deserializer)?;
        Ok(Self(Duration::from_secs(seconds.get())))
    }
}

/// A string that is not empty or whitespace, kept as written.
#[derive(Clone, Serialize)]
#[serde(transparent)]
struct NonBlank(String);

impl<'de> Deserialize<'de> for NonBlank {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        if value.trim().is_empty() {
            return Err(D::Error::custom("must not be empty"));
        }
        Ok(Self(value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(json: serde_json::Value) -> Result<Config, serde_json::Error> {
        serde_json::from_value(json)
    }

    #[test]
    fn rejects_blank_credentials_and_a_zero_timeout() {
        let config = parse(serde_json::json!({
            "oauth": { "client_id": "client", "client_secret": "secret" },
            "outbound_http_timeout_seconds": 25
        }))
        .unwrap();
        assert_eq!(config.timeout(), Duration::from_secs(25));
        assert!(config.client().unwrap().is_some());

        assert!(
            parse(serde_json::json!({ "oauth": { "client_id": " ", "client_secret": "s" } }))
                .is_err()
        );
        assert!(parse(serde_json::json!({ "outbound_http_timeout_seconds": 0 })).is_err());
    }
}
