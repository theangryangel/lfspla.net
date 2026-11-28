//! Upload policy, LFS installations and background processing for hotlaps.

use std::{
    num::NonZeroU64,
    path::{Path, PathBuf},
};

use anyhow::ensure;
use lfsplanet_core::settings::ConfigPath;
use lfsplanet_lfs_installations::RuntimeConfig;
use serde::{Deserialize, Serialize};
use size::Size;

use super::{ValidationSettings, WebhookSettings};

/// Upload limits, local LFS installations, validation and webhook delivery.
#[derive(Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct HotlapSettings {
    /// Whether the API accepts new hotlap uploads globally.
    pub allow_uploads: bool,
    /// Whether uploaded replay usernames must match the authenticated player.
    pub enforce_replay_username: bool,
    /// Maximum number of unfinished hotlaps allowed per player.
    pub max_queued_per_player: NonZeroU64,
    /// Maximum size of an uploaded SPR file, such as `"2 MiB"`.
    #[serde(default = "default_max_spr_upload_bytes")]
    pub max_spr_upload_bytes: Size,
    /// Whether the temporary immediate-validation endpoint is enabled.
    pub allow_test_validation: bool,
    /// Root containing persistent named LFS installations.
    pub installation_root: ConfigPath,
    /// Platform runtime used to install, update and validate with LFS.
    pub runtime: RuntimeConfig,
    /// Scheduling and deadlines for queued replay validation.
    pub validation: ValidationSettings,
    /// Outgoing webhook delivery.
    pub webhooks: WebhookSettings,
}

impl Default for HotlapSettings {
    fn default() -> Self {
        Self {
            allow_uploads: true,
            enforce_replay_username: true,
            max_queued_per_player: NonZeroU64::new(10).expect("10 is non-zero"),
            max_spr_upload_bytes: default_max_spr_upload_bytes(),
            allow_test_validation: false,
            installation_root: ConfigPath::new("/srv/lfs"),
            runtime: RuntimeConfig::default(),
            validation: ValidationSettings::default(),
            webhooks: WebhookSettings::default(),
        }
    }
}

impl HotlapSettings {
    /// Returns the configured upload limit after configuration validation.
    pub fn max_spr_upload_bytes(&self) -> usize {
        usize::try_from(self.max_spr_upload_bytes.bytes())
            .expect("max SPR upload size was validated when configuration loaded")
    }

    /// Installer cache shared by every named installation.
    pub fn installer_cache_root(&self) -> PathBuf {
        self.installation_root.path().join(".cache/installers")
    }

    /// Resolves relative installation and runtime paths against the config directory.
    pub fn resolve_paths(&mut self, base: &Path) -> anyhow::Result<()> {
        self.installation_root.resolve(base);
        self.runtime.resolve_paths(base)
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        ensure!(
            self.max_spr_upload_bytes.bytes() > 0,
            "hotlaps.max_spr_upload_bytes must be greater than zero"
        );
        ensure!(
            usize::try_from(self.max_spr_upload_bytes.bytes()).is_ok(),
            "hotlaps.max_spr_upload_bytes is too large for this host"
        );
        Ok(())
    }
}

fn default_max_spr_upload_bytes() -> Size {
    Size::from_mib(2)
}
