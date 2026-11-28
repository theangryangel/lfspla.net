//! Manage configured LFS installations using the standalone runtime crate.

use crate::installation_id::InstallationId;
mod add;
mod installations;

pub use add::{AddInstallation, add};
pub use installations::{installation_path, resolve_installation};

use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::ensure;
use lfsplanet_lfs_installations::{TrimPlan, TrimSummary};

use crate::settings::HotlapSettings;

/// Update an existing named installation.
pub async fn update(
    settings: &HotlapSettings,
    installation_id: &InstallationId,
    command_timeout: Duration,
) -> anyhow::Result<PathBuf> {
    ensure!(
        !command_timeout.is_zero(),
        "command timeout must be greater than zero"
    );
    lfsplanet_lfs_installations::ensure_supported()?;
    let installation_dir =
        resolve_installation(settings.installation_root.path(), installation_id)?;
    tracing::info!(%installation_id, path = %installation_dir.display(), "updating LFS installation");
    lfsplanet_lfs_installations::update(&settings.runtime, &installation_dir, command_timeout)
        .await?;
    Ok(installation_dir)
}

/// Prepare a trim, obtain the caller's review, then execute the reviewed plan.
pub fn trim(
    installation_dir: &Path,
    dds: bool,
    lgh: bool,
    review: impl FnOnce(&TrimPlan) -> anyhow::Result<()>,
) -> anyhow::Result<TrimSummary> {
    let plan = TrimPlan::new(installation_dir, dds, lgh)?;
    review(&plan)?;
    lfsplanet_lfs_installations::trim(plan)
}
