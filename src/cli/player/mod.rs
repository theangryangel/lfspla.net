//! Operator CLI for inspecting and changing player account restrictions.

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

use crate::{cli::Args, db, settings::Settings};
use anyhow::Context;

/// Runs one player account command.
pub(crate) async fn run(args: &Args, action: &PlayerCommand) -> anyhow::Result<()> {
    let settings = Settings::load(&args.config)?;
    let database = db::connect(&settings.database, 2).await?;
    let lfs_username = match action {
        PlayerCommand::Deny { lfs_username, .. }
        | PlayerCommand::Allow { lfs_username, .. }
        | PlayerCommand::Show { lfs_username } => lfs_username,
    };
    let player = crate::models::Player::find_by_username(&database, lfs_username)
        .await?
        .with_context(|| format!("player {lfs_username:?} was not found"))?;

    match action {
        PlayerCommand::Show { .. } => {
            println!(
                "{}\tdeny_auth={}\tdeny_uploads={}",
                player.lfs_username, player.deny_auth, player.deny_uploads
            );
        }
        PlayerCommand::Deny { restriction, .. } | PlayerCommand::Allow { restriction, .. } => {
            let denied = matches!(action, PlayerCommand::Deny { .. });
            let (currently_denied, label) = match restriction {
                PlayerRestriction::Auth => (player.deny_auth, "authentication"),
                PlayerRestriction::Uploads => (player.deny_uploads, "uploads"),
            };
            let disposition = if denied { "denied" } else { "allowed" };
            if currently_denied == denied {
                println!(
                    "Player {:?}: {label} already {disposition}.",
                    player.lfs_username
                );
                return Ok(());
            }

            let player = match restriction {
                PlayerRestriction::Auth => player.set_auth_denied(&database, denied).await?,
                PlayerRestriction::Uploads => player.set_uploads_denied(&database, denied).await?,
            };
            println!("Player {:?}: {label} {disposition}.", player.lfs_username);
        }
    }

    Ok(())
}
