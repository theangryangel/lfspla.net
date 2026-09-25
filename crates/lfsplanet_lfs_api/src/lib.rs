//! Client support for the remote Live for Speed OAuth and HTTP APIs.

mod oauth;
mod vehicle_mods;

pub use oauth::{LfsAccount, OAuthProvider};
pub use vehicle_mods::{CoverImage, RemoteVehicleMod};

use std::{sync::Arc, time::Duration};

use serde::Deserialize;
use tokio::sync::Mutex;

pub(crate) mod endpoints {
    pub(crate) const AUTHORIZE: &str = "https://id.lfs.net/oauth2/authorize";
    pub(crate) const TOKEN: &str = "https://id.lfs.net/oauth2/access_token";
    pub(crate) const USERINFO: &str = "https://api.lfs.net/userinfo";
    pub(crate) const VEHICLE_MODS: &str = "https://api.lfs.net/vehiclemod";
}

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const TOKEN_RENEWAL_MARGIN: u64 = 30;

/// Builds a client for one bounded request to LFS.
pub fn http_client(timeout: Duration) -> reqwest::Result<reqwest::Client> {
    identified_client()
        .connect_timeout(timeout.min(CONNECT_TIMEOUT))
        .timeout(timeout)
        .build()
}

/// Builds a client for streaming a large file from LFS.
pub fn download_client(connect_timeout: Duration) -> reqwest::Result<reqwest::Client> {
    identified_client().connect_timeout(connect_timeout).build()
}

fn identified_client() -> reqwest::ClientBuilder {
    reqwest::Client::builder().user_agent(concat!("lfsplanet/", env!("CARGO_PKG_VERSION")))
}

/// A failed exchange with LFS.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// LFS rejected the request, or it never completed.
    #[error(transparent)]
    Http(#[from] reqwest::Error),
    /// LFS returned a profile this service could not read.
    #[error("LFS returned an unreadable profile")]
    InvalidUserInfo(#[source] serde_json::Error),
    /// LFS returned a profile without an account username.
    #[error("LFS did not return a username")]
    MissingUsername,
    /// LFS returned a Vehicle Mods catalogue this service could not read.
    #[error("LFS Vehicle Mods API returned an invalid catalogue: {0}")]
    InvalidCatalogue(String),
}

/// A credentialed client for LFS HTTP APIs.
///
/// Cheap to clone: clones share one connection pool and one cached service token.
#[derive(Clone)]
pub struct LfsClient {
    http: reqwest::Client,
    client_id: String,
    client_secret: String,
    cached_service_token: Arc<Mutex<Option<CachedToken>>>,
}

struct CachedToken {
    access_token: String,
    expires_at: tokio::time::Instant,
}

impl LfsClient {
    /// Builds a client from an HTTP client and LFS API credentials.
    #[must_use]
    pub fn new(http: reqwest::Client, client_id: String, client_secret: String) -> Self {
        Self {
            http,
            client_id,
            client_secret,
            cached_service_token: Arc::default(),
        }
    }

    async fn service_token(&self) -> Result<String, Error> {
        let mut cached = self.cached_service_token.lock().await;
        if let Some(token) = cached.as_ref()
            && token.expires_at > tokio::time::Instant::now()
        {
            return Ok(token.access_token.clone());
        }
        let response = self
            .token(&[
                ("grant_type", "client_credentials"),
                ("client_id", self.client_id.as_str()),
                ("client_secret", self.client_secret.as_str()),
            ])
            .await?;
        let lifetime =
            Duration::from_secs(response.expires_in.saturating_sub(TOKEN_RENEWAL_MARGIN));
        *cached = Some(CachedToken {
            access_token: response.access_token.clone(),
            expires_at: tokio::time::Instant::now() + lifetime,
        });
        Ok(response.access_token)
    }

    async fn token(&self, grant: &[(&str, &str)]) -> Result<TokenResponse, Error> {
        Ok(self
            .http
            .post(endpoints::TOKEN)
            .form(grant)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?)
    }
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    expires_in: u64,
}
