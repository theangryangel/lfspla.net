//! Arguments for player account operator commands.

use clap::{Subcommand, ValueEnum};

/// Player account restriction operation.
#[derive(Clone, Debug, Eq, PartialEq, Subcommand)]
pub(crate) enum PlayerCommand {
    /// Deny one operation to a player.
    Deny {
        /// Operation to deny.
        #[arg(value_enum, value_name = "RESTRICTION")]
        restriction: PlayerRestriction,
        /// Exact LFS account name.
        #[arg(value_name = "LFS_USERNAME")]
        lfs_username: String,
    },
    /// Allow one previously denied operation for a player.
    Allow {
        /// Operation to allow.
        #[arg(value_enum, value_name = "RESTRICTION")]
        restriction: PlayerRestriction,
        /// Exact LFS account name.
        #[arg(value_name = "LFS_USERNAME")]
        lfs_username: String,
    },
    /// Show the current restrictions for a player.
    Show {
        /// Exact LFS account name.
        #[arg(value_name = "LFS_USERNAME")]
        lfs_username: String,
    },
}

/// An independently configurable player account restriction.
#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(crate) enum PlayerRestriction {
    /// All browser-session and personal-access-token authentication.
    Auth,
    /// New hotlap uploads.
    Uploads,
}
