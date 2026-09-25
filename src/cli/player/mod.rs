//! Operator CLI for inspecting and changing player account restrictions.

mod args;

pub(crate) use args::PlayerCommand;
use args::PlayerRestriction;

use anyhow::Context;
use sea_orm::{ActiveModelTrait, EntityTrait, IntoActiveModel, Set};

use crate::{
    cli::Args,
    models::players::{PlayerEntity, PlayerFilter},
    settings::Settings,
    startup,
};

/// Runs one player account command.
pub(crate) async fn run(args: &Args, action: &PlayerCommand) -> anyhow::Result<()> {
    let settings = Settings::load(&args.config)?;
    let database = startup::connect(&settings.database, 2).await?;
    let lfs_username = match action {
        PlayerCommand::Deny { lfs_username, .. }
        | PlayerCommand::Allow { lfs_username, .. }
        | PlayerCommand::Show { lfs_username } => lfs_username,
    };
    let player = PlayerEntity::find()
        .with_username(lfs_username)
        .one(&database)
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

            let mut active = player.into_active_model();
            match restriction {
                PlayerRestriction::Auth => active.deny_auth = Set(denied),
                PlayerRestriction::Uploads => active.deny_uploads = Set(denied),
            }
            let player = active.update(&database).await?;
            println!("Player {:?}: {label} {disposition}.", player.lfs_username);
        }
    }

    Ok(())
}
