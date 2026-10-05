//! Settings for local LFS installations and outbound integrations.

use lfsplanet_lfs_api::LfsClient;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use super::types::{ConfigPath, NonEmptyString, Seconds};

/// Installation locations, platform runtime, credentials and HTTP limits.
#[derive(Clone, Deserialize, Serialize)]
#[serde(default)]
#[cfg_attr(target_os = "linux", serde(deny_unknown_fields))]
pub struct LfsSettings {
    /// Root containing persistent named installations.
    pub installation_root: ConfigPath,
    /// Platform-owned fields are flattened within the LFS section.
    #[serde(flatten)]
    pub runtime: lfsplanet_lfs::RuntimeConfig,
    /// OAuth client credentials, when the integration is configured.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oauth: Option<OAuthSettings>,
    /// Maximum time allowed for one outbound LFS HTTP request.
    #[serde(rename = "outbound_http_timeout_seconds")]
    pub outbound_http_timeout: Seconds<15>,
}

impl Default for LfsSettings {
    fn default() -> Self {
        Self {
            installation_root: ConfigPath::new("/srv/lfs"),
            runtime: lfsplanet_lfs::RuntimeConfig::default(),
            oauth: None,
            outbound_http_timeout: Seconds::default(),
        }
    }
}

impl LfsSettings {
    /// Creates an LFS API client when OAuth credentials are configured.
    pub(crate) fn to_api_client(&self) -> anyhow::Result<Option<LfsClient>> {
        let Some(oauth) = &self.oauth else {
            return Ok(None);
        };
        Ok(Some(LfsClient::new(
            lfsplanet_lfs_api::http_client(self.outbound_http_timeout.duration())?,
            oauth.client_id.as_str().to_owned(),
            oauth.client_secret.as_str().to_owned(),
        )))
    }

    /// Installer cache shared by every named installation.
    pub fn installer_cache_root(&self) -> PathBuf {
        self.installation_root.path().join(".cache/installers")
    }

    pub(super) fn resolve_paths(&mut self, base: &Path) -> anyhow::Result<()> {
        self.installation_root.resolve(base);
        self.runtime.resolve_paths(base)
    }
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
