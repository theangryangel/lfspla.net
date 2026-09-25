//! Arguments for LFS installation operator commands.

use clap::Subcommand;

/// LFS validator installation operation.
#[derive(Clone, Debug, Eq, PartialEq, Subcommand)]
pub(crate) enum LfsCommand {
    /// Print the configured path for a named LFS installation.
    Path {
        /// Installation identifier.
        #[arg(value_name = "INSTALLATION")]
        installation_id: String,
    },
    /// Create, prepare, and unlock a named LFS installation.
    Install {
        /// New installation identifier.
        #[arg(value_name = "INSTALLATION")]
        installation_id: String,
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
        installation_id: String,
        /// Deadline for the LFS update process.
        #[arg(long, value_name = "SECONDS", default_value_t = 1800)]
        command_timeout_seconds: u64,
    },
    /// Permanently remove selected data from a named LFS installation.
    Trim {
        /// Existing installation identifier to trim.
        #[arg(value_name = "INSTALLATION")]
        installation_id: String,
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
