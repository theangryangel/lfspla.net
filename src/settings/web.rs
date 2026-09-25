//! Settings for the HTTP frontend of the API process.

use std::net::SocketAddr;

use serde::{Deserialize, Serialize};
use url::Url;

use super::types::{PublicBaseUrl, Seconds, SessionKey};

/// How the API process presents itself over HTTP.
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WebSettings {
    /// Encryption key for the private session cookie. Required; no default.
    pub session_key: SessionKey,
    /// Socket on which the API listens.
    #[serde(default = "default_listen")]
    pub listen: SocketAddr,
    /// Public URL used for OAuth callback links.
    #[serde(default = "default_public_base_url")]
    pub public_base_url: PublicBaseUrl,
    /// Whether the browser should only send the session cookie over HTTPS.
    #[serde(default = "default_cookie_secure")]
    pub cookie_secure: bool,
    /// Legacy frontend setting, accepted for configuration compatibility.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub site_notice: Option<String>,
    /// Maximum time allowed for one inbound HTTP request.
    #[serde(default, rename = "http_request_timeout_seconds")]
    pub http_request_timeout: Seconds<30>,
}

impl Default for WebSettings {
    fn default() -> Self {
        Self {
            session_key: SessionKey::default(),
            listen: default_listen(),
            public_base_url: default_public_base_url(),
            cookie_secure: default_cookie_secure(),
            site_notice: None,
            http_request_timeout: Seconds::default(),
        }
    }
}

impl WebSettings {
    /// Returns the callback URL registered with LFS OAuth.
    pub fn oauth_callback_url(&self) -> Url {
        self.public_base_url
            .url()
            .join("/auth/lfs/callback")
            .expect("the fixed callback path is a valid URL")
    }
}

fn default_listen() -> SocketAddr {
    SocketAddr::from(([0, 0, 0, 0], 8000))
}

fn default_public_base_url() -> PublicBaseUrl {
    serde_saphyr::from_str("http://localhost:8000").expect("the default public base URL is valid")
}

const fn default_cookie_secure() -> bool {
    true
}
