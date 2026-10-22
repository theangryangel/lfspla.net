//! One-shot maintenance command orchestration.

use crate::{
    cli::{Args, storage},
    db,
    settings::Settings,
};
use anyhow::{Context, bail};
use clap::Subcommand;
use sea_orm::DatabaseConnection;
use time::Duration;
use tower_sessions::session_store::ExpiredDeletion;
use tower_sessions_sqlx_store::PostgresStore;
const PERSONAL_ACCESS_TOKEN_RETENTION: Duration = Duration::days(90);

/// One-shot maintenance operation.
#[derive(Clone, Debug, Eq, PartialEq, Subcommand)]
pub(crate) enum MaintenanceCommand {
    /// Delete expired browser sessions.
    SessionsGc,
    /// Delete access tokens retained past expiry or revocation.
    AccessTokensGc,
    /// Synchronize Vehicle Mods metadata and covers from the LFS API.
    SyncMods,
    /// Import public LFS event advertisements, or preview proposed changes.
    SyncEvents {
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
    /// Find attachment objects without references.
    StorageGc {
        /// Delete eligible objects instead of only reporting them.
        #[arg(long)]
        delete: bool,
        /// Minimum age of an unreferenced object before it is eligible.
        #[arg(long, value_name = "HOURS", default_value_t = 24)]
        older_than_hours: u64,
    },
    /// Run routine maintenance tasks once.
    RunAll {
        /// Delete eligible storage objects instead of only reporting them.
        #[arg(long)]
        delete: bool,
        /// Minimum age of an unreferenced storage object before it is eligible.
        #[arg(long, value_name = "HOURS", default_value_t = 24)]
        older_than_hours: u64,
    },
}

/// Runs the selected maintenance task once and exits.
pub(crate) async fn run(args: &Args, action: &MaintenanceCommand) -> anyhow::Result<()> {
    if let MaintenanceCommand::SyncEvents {
        league,
        source_only: true,
        ..
    } = action
    {
        let report = crate::services::sync_events::preview(*league).await?;
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

    match action {
        MaintenanceCommand::SyncEvents {
            league, dry_run, ..
        } => sync_events(&settings, &database, *league, *dry_run).await,
        MaintenanceCommand::SessionsGc => sessions_gc(&database).await,
        MaintenanceCommand::AccessTokensGc => access_tokens_gc(&database).await,
        MaintenanceCommand::SyncMods => {
            crate::services::catalogue::sync_mods(&settings, &database).await
        }
        MaintenanceCommand::StorageGc {
            delete,
            older_than_hours,
        } => storage::gc(&settings.storage, &database, *delete, *older_than_hours).await,
        MaintenanceCommand::RunAll {
            delete,
            older_than_hours,
        } => run_all(&settings, &database, *delete, *older_than_hours).await,
    }
}

async fn sync_events(
    settings: &Settings,
    database: &DatabaseConnection,
    league: Option<u64>,
    dry_run: bool,
) -> anyhow::Result<()> {
    let report =
        crate::services::sync_events::sync(database, league, dry_run, &settings.storage).await?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    if !report.failures.is_empty() {
        bail!(
            "event sync had {} failure(s); see JSON report",
            report.failures.len()
        );
    }
    Ok(())
}

async fn sessions_gc(database: &DatabaseConnection) -> anyhow::Result<()> {
    let store = PostgresStore::new(database.get_postgres_connection_pool().clone());
    store
        .delete_expired()
        .await
        .context("failed to delete expired sessions")?;
    tracing::info!("expired session garbage collection completed");
    Ok(())
}

async fn access_tokens_gc(database: &DatabaseConnection) -> anyhow::Result<()> {
    let cutoff = time::OffsetDateTime::now_utc() - PERSONAL_ACCESS_TOKEN_RETENTION;
    let deleted = crate::models::PersonalAccessToken::delete_stale(database, cutoff)
        .await
        .context("failed to delete stale personal access tokens")?;
    tracing::info!(
        deleted,
        "stale personal access token garbage collection completed"
    );
    Ok(())
}

async fn run_all(
    settings: &Settings,
    database: &DatabaseConnection,
    delete_storage: bool,
    storage_older_than_hours: u64,
) -> anyhow::Result<()> {
    let mut failures = Vec::new();

    // Every task runs even when an earlier one fails; the run reports them all.
    let mut record = |name: &str, result: anyhow::Result<()>| {
        if let Err(error) = result {
            tracing::error!(?error, task = name, "maintenance task failed");
            failures.push(format!("{name}: {error:#}"));
        }
    };

    record("sessions-gc", sessions_gc(database).await);
    record("access-tokens-gc", access_tokens_gc(database).await);
    record(
        "badges-refresh",
        crate::models::badge::retry_badge_refreshes(database)
            .await
            .map_err(Into::into),
    );
    record(
        "storage-gc",
        storage::gc(
            &settings.storage,
            database,
            delete_storage,
            storage_older_than_hours,
        )
        .await,
    );
    record(
        "sync-mods",
        crate::services::catalogue::sync_mods(settings, database).await,
    );

    record(
        "sync-events",
        sync_events(settings, database, None, false).await,
    );

    if failures.is_empty() {
        Ok(())
    } else {
        bail!("maintenance tasks failed: {}", failures.join("; "))
    }
}
