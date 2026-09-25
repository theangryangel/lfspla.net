//! Settings for outbound calls to LFS.

use serde::{Deserialize, Serialize};

use super::types::{NonEmptyString, Seconds};

/// Credentials and HTTP timeouts for OAuth, Vehicle Mods, and LFS downloads.
#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct LfsSettings {
    /// OAuth client credentials, when the integration is configured.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oauth: Option<OAuthSettings>,
    /// Maximum time allowed for one outbound LFS HTTP request.
    #[serde(rename = "outbound_http_timeout_seconds")]
    pub outbound_http_timeout: Seconds<15>,
}

/// LFS OAuth client credentials.
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OAuthSettings {
    /// OAuth client identifier issued by LFS.
    pub client_id: NonEmptyString,
    /// OAuth client secret issued by LFS.
    pub client_secret: NonEmptyString,
}
