//! Garbage collection of expired sessions, stale access tokens and unreferenced storage.

use crate::{
    cli::{Args, storage},
    settings::Settings,
};
use anyhow::{Context, bail};
use clap::{ArgGroup, value_parser};
use lfsplanet_core::db;
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use tower_sessions::session_store::ExpiredDeletion;
use tower_sessions_sqlx_store::PostgresStore;

/// What to collect, how old it must be, and whether to delete it.
#[derive(Clone, Debug, Eq, PartialEq, clap::Args)]
#[command(group(
    ArgGroup::new("targets")
        .required(true)
        .multiple(true)
        .args(["sessions", "access_tokens", "storage", "all"])
))]
#[command(group(ArgGroup::new("mode").required(true).args(["yes", "dry_run"])))]
pub(crate) struct GcArgs {
    /// Expired browser sessions.
    #[arg(long)]
    sessions: bool,
    /// Access tokens revoked or expired longer ago than --access-token-max-age-days.
    #[arg(long)]
    access_tokens: bool,
    /// Attachment objects without references, older than --storage-max-age-hours.
    #[arg(long)]
    storage: bool,
    /// Every target.
    #[arg(long)]
    all: bool,
    /// Days a revoked or expired access token is retained.
    #[arg(long, value_name = "DAYS", default_value_t = 90, value_parser = value_parser!(u32).range(1..))]
    access_token_max_age_days: u32,
    /// Minimum age of an unreferenced storage object before it is eligible.
    #[arg(long, value_name = "HOURS", default_value_t = 24)]
    storage_max_age_hours: u64,
    /// Delete everything eligible.
    #[arg(long)]
    yes: bool,
    /// Report what is eligible without deleting it.
    #[arg(long)]
    dry_run: bool,
}

/// Collects each selected target; every target runs even when an earlier one fails.
pub(crate) async fn run(args: &Args, options: &GcArgs) -> anyhow::Result<()> {
    let settings = Settings::load(&args.config)?;
    let database = db::connect(&settings.database, 2).await?;
    let delete = options.yes;
    let mut failures = Vec::new();
    let mut record = |name: &str, result: anyhow::Result<()>| {
        if let Err(error) = result {
            tracing::error!(?error, target = name, "garbage collection failed");
            failures.push(format!("{name}: {error:#}"));
        }
    };
    if options.all || options.sessions {
        record("sessions", sessions(&database, delete).await);
    }
    if options.all || options.access_tokens {
        record(
            "access-tokens",
            access_tokens(&database, options.access_token_max_age_days, delete).await,
        );
    }
    if options.all || options.storage {
        record(
            "storage",
            storage::gc(
                &settings.storage,
                &database,
                delete,
                options.storage_max_age_hours,
            )
            .await,
        );
    }
    if failures.is_empty() {
        Ok(())
    } else {
        bail!("garbage collection failed: {}", failures.join("; "))
    }
}

async fn sessions(database: &DatabaseConnection, delete: bool) -> anyhow::Result<()> {
    if delete {
        PostgresStore::new(database.get_postgres_connection_pool().clone())
            .delete_expired()
            .await
            .context("failed to delete expired sessions")?;
        tracing::info!("expired sessions deleted");
        return Ok(());
    }
    // Matches the session store's own expiry rule.
    let row = database
        .query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            "SELECT count(*) AS expired FROM tower_sessions.session \
             WHERE expiry_date < (now() at time zone 'utc')",
        ))
        .await?
        .context("session count returned no row")?;
    let expired: i64 = row.try_get("", "expired")?;
    println!("sessions\twould-delete\t{expired}");
    Ok(())
}

async fn access_tokens(
    database: &DatabaseConnection,
    max_age_days: u32,
    delete: bool,
) -> anyhow::Result<()> {
    let cutoff = time::OffsetDateTime::now_utc() - time::Duration::days(max_age_days.into());
    if delete {
        let deleted = crate::personal_access_token::Model::delete_stale(database, cutoff)
            .await
            .context("failed to delete stale personal access tokens")?;
        tracing::info!(deleted, "stale personal access tokens deleted");
    } else {
        let stale = crate::personal_access_token::Model::count_stale(database, cutoff)
            .await
            .context("failed to count stale personal access tokens")?;
        println!("access-tokens\twould-delete\t{stale}");
    }
    Ok(())
}
