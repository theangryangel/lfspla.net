//! Policy governing uploaded hotlaps.

use std::num::NonZeroU64;

use anyhow::ensure;
use serde::{Deserialize, Serialize};
use size::Size;

/// Limits and checks applied to uploaded hotlaps.
#[derive(Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct HotlapSettings {
    /// Whether uploaded replay usernames must match the authenticated player.
    pub enforce_replay_username: bool,
    /// Maximum number of unfinished hotlaps allowed per player.
    pub max_queued_per_player: NonZeroU64,
    /// Maximum size of an uploaded SPR file, such as `"2 MiB"`.
    #[serde(default = "default_max_spr_upload_bytes")]
    pub max_spr_upload_bytes: Size,
    /// Whether the temporary immediate-validation endpoint is enabled.
    pub allow_test_validation: bool,
}

impl Default for HotlapSettings {
    fn default() -> Self {
        Self {
            enforce_replay_username: true,
            max_queued_per_player: NonZeroU64::new(10).expect("10 is non-zero"),
            max_spr_upload_bytes: default_max_spr_upload_bytes(),
            allow_test_validation: false,
        }
    }
}

impl HotlapSettings {
    /// Returns the configured upload limit after configuration validation.
    pub fn max_spr_upload_bytes(&self) -> usize {
        usize::try_from(self.max_spr_upload_bytes.bytes())
            .expect("max SPR upload size was validated when configuration loaded")
    }

    pub(super) fn validate(&self) -> anyhow::Result<()> {
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
