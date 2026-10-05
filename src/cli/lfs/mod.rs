//! Operator CLI for adding and maintaining named LFS installations.

use crate::installation_id::InstallationId;
use clap::Subcommand;

/// LFS validator installation operation.
#[derive(Clone, Debug, Eq, PartialEq, Subcommand)]
pub(crate) enum LfsCommand {
    /// Print the configured path for a named LFS installation.
    Path {
        /// Installation identifier.
        #[arg(value_name = "INSTALLATION")]
        installation_id: InstallationId,
    },
    /// Add a named LFS installation, preparing and unlocking it.
    Add {
        /// New installation identifier.
        #[arg(value_name = "INSTALLATION")]
        installation_id: InstallationId,
        /// URL of the full LFS NSIS installer.
        #[arg(long, value_name = "URL")]
        download_url: String,
        /// Licensed LFS user name used to unlock this installation.
        #[arg(long)]
        username: String,
        /// LFS unlock code.
        #[arg(long, value_name = "UNLOCK")]
        unlock_code: String,
        /// Deadline for each installer, preparation, or unlock process.
        #[arg(long, value_name = "SECONDS", default_value_t = 1800)]
        command_timeout_seconds: u64,
    },
    /// Update an existing named LFS installation from lfs.net.
    Update {
        /// Existing installation identifier to update.
        #[arg(value_name = "INSTALLATION")]
        installation_id: InstallationId,
        /// Deadline for the LFS update process.
        #[arg(long, value_name = "SECONDS", default_value_t = 1800)]
        command_timeout_seconds: u64,
    },
    /// Permanently remove selected data from a named LFS installation.
    Trim {
        /// Existing installation identifier to trim.
        #[arg(value_name = "INSTALLATION")]
        installation_id: InstallationId,
        /// Delete every entry beneath data/dds, retaining the directory.
        #[arg(long)]
        dds: bool,
        /// Delete *.lgh files directly beneath data/wld.
        #[arg(long)]
        lgh: bool,
        /// Confirm permanent deletion of the selected data.
        #[arg(long)]
        yes: bool,
    },
}

mod trim;

use crate::{
    cli::Args,
    services::manage_lfs::{self, AddInstallation},
    settings::Settings,
};
use std::time::Duration;

/// Runs one LFS installation command.
pub(crate) async fn run(args: &Args, action: &LfsCommand) -> anyhow::Result<()> {
    let settings = Settings::load(&args.config)?;
    match action {
        LfsCommand::Path { installation_id } => {
            let path = manage_lfs::installation_path(
                settings.lfs.installation_root.path(),
                installation_id,
            )?;
            println!("{}", path.display());
        }
        LfsCommand::Add {
            installation_id,
            download_url,
            username,
            unlock_code,
            command_timeout_seconds,
        } => {
            let installation_dir = manage_lfs::add(
                &settings.lfs,
                AddInstallation {
                    installation_id,
                    download_url,
                    username,
                    unlock_code,
                    command_timeout: Duration::from_secs(*command_timeout_seconds),
                },
            )
            .await?;
            tracing::info!(%installation_id, path = %installation_dir.display(), "LFS installation completed");
        }
        LfsCommand::Update {
            installation_id,
            command_timeout_seconds,
        } => {
            let installation_dir = manage_lfs::update(
                &settings.lfs,
                installation_id,
                Duration::from_secs(*command_timeout_seconds),
            )
            .await?;
            tracing::info!(%installation_id, path = %installation_dir.display(), "LFS installation updated");
        }
        LfsCommand::Trim {
            installation_id,
            dds,
            lgh,
            yes,
        } => {
            trim::run(&settings, installation_id, *dds, *lgh, *yes)?;
        }
    }
    Ok(())
}
