//! Update an existing LFS installation.

use std::time::Duration;

use anyhow::ensure;

use crate::{cli::Args, lfs::installations::resolve_installation, settings::Settings};
use lfsplanet_lfs::{Command as SandboxCommand, executable, validate_prefix};

use super::checked_output;

pub(super) async fn run(
    args: &Args,
    installation_id: &str,
    command_timeout_seconds: u64,
) -> anyhow::Result<()> {
    ensure!(
        command_timeout_seconds > 0,
        "command timeout must be greater than zero"
    );

    let settings = Settings::load(&args.config)?;
    let installation_dir = resolve_installation(
        settings.lfs_runtime.installation_root.path(),
        installation_id,
    )?;
    let wine_prefix = validate_prefix(settings.lfs_runtime.wine_prefix.path())?;

    let bubblewrap = executable(
        settings.lfs_runtime.bubblewrap_executable.path(),
        "Bubblewrap",
    )?;
    let wine = executable(settings.lfs_runtime.wine_executable.path(), "Wine")?;

    tracing::info!(installation_id, path = %installation_dir.display(), "updating LFS installation");
    checked_output(
        SandboxCommand::Update {
            installation_dir: &installation_dir,
            wine_prefix: &wine_prefix,
        }
        .build(&bubblewrap, &wine),
        Duration::from_secs(command_timeout_seconds),
        "LFS update",
        None,
    )
    .await?;
    tracing::info!(installation_id, path = %installation_dir.display(), "LFS installation updated");
    Ok(())
}
