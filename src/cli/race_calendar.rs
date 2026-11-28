//! Operator commands owned by the race calendar crate.

use crate::{cli::Args, settings::Settings};
use anyhow::bail;
use clap::Subcommand;
use lfsplanet_core::db;
use lfsplanet_race_calendar::services::sync_events;
use sea_orm::DatabaseConnection;

/// Race calendar operation.
#[derive(Clone, Debug, Eq, PartialEq, Subcommand)]
pub(crate) enum RaceCalendarCommand {
    /// Import public LFS event advertisements, or preview proposed changes.
    Sync {
        /// Compare with stored records and roll back all proposed changes.
        #[arg(long, conflicts_with = "source_only")]
        dry_run: bool,
        /// Print upcoming source records without loading configuration or a database.
        #[arg(long, conflicts_with = "dry_run")]
        source_only: bool,
        /// Inspect one LFS league instead of the full active directory.
        #[arg(long, value_parser = clap::value_parser!(u64).range(1..))]
        league: Option<u64>,
    },
}

/// Runs one race calendar command.
pub(crate) async fn run(args: &Args, action: &RaceCalendarCommand) -> anyhow::Result<()> {
    let RaceCalendarCommand::Sync {
        dry_run,
        source_only,
        league,
    } = action;
    if *source_only {
        let report = sync_events::preview(*league).await?;
        println!("{}", serde_json::to_string_pretty(&report)?);
        if !report.failures.is_empty() {
            bail!(
                "event preview had {} failure(s); see JSON report",
                report.failures.len()
            );
        }
        return Ok(());
    }
    let settings = Settings::load(&args.config)?;
    let database = db::connect(&settings.database, 2).await?;
    sync(&settings, &database, *league, *dry_run).await
}

/// Imports event advertisements.
pub(crate) async fn sync(
    settings: &Settings,
    database: &DatabaseConnection,
    league: Option<u64>,
    dry_run: bool,
) -> anyhow::Result<()> {
    let report = sync_events::sync(database, league, dry_run, &settings.storage).await?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    if !report.failures.is_empty() {
        bail!(
            "event sync had {} failure(s); see JSON report",
            report.failures.len()
        );
    }
    Ok(())
}
