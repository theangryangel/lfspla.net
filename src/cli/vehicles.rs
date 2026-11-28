//! Vehicle commands, including Vehicle Mod synchronisation from the LFS API.

use crate::{cli::Args, settings::Settings};
use anyhow::Context;
use clap::Subcommand;
use lfsplanet_core::db;
use sea_orm::DatabaseConnection;

/// Vehicle operation.
#[derive(Clone, Debug, Eq, PartialEq, Subcommand)]
pub(crate) enum VehiclesCommand {
    /// Synchronize Vehicle Mods metadata and covers from the LFS API.
    SyncMods,
}

/// Runs one vehicle command.
pub(crate) async fn run(args: &Args, action: &VehiclesCommand) -> anyhow::Result<()> {
    let VehiclesCommand::SyncMods = action;
    let settings = Settings::load(&args.config)?;
    let database = db::connect(&settings.database, 2).await?;
    sync_mods(&settings, &database).await
}

/// Synchronises Vehicle Mods.
pub(crate) async fn sync_mods(
    settings: &Settings,
    database: &DatabaseConnection,
) -> anyhow::Result<()> {
    let client = settings.lfs_net.client()?.context(
        "LFS OAuth credentials are required for Vehicle Mod synchronization; configure lfs_net.oauth",
    )?;
    lfsplanet_hotlaps::services::catalogue::sync_mods(client, &settings.storage, database).await
}
