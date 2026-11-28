//! Shared process db.
//!
//! Shared database connection, migration, and catalogue locking operations.

pub mod escaped_like;

use crate::settings::DatabaseSettings;
use anyhow::Context;
use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use sqlx::migrate::Migrator;
pub static MIGRATOR: Migrator = sqlx::migrate!("../../migrations");

/// Serializes catalogue changes with admission and validation decisions.
pub const CATALOGUE_LOCK: i64 = 7_104_152_026;

/// Allows concurrent catalogue readers while excluding catalogue writes.
/// Call inside the owning transaction; released when that transaction ends.
pub async fn lock_catalogue_read<C: sea_orm::ConnectionTrait>(
    database: &C,
) -> Result<(), sea_orm::DbErr> {
    database
        .query_one_raw(sea_orm::Statement::from_sql_and_values(
            sea_orm::DbBackend::Postgres,
            "SELECT pg_advisory_xact_lock_shared($1)",
            [CATALOGUE_LOCK.into()],
        ))
        .await?;
    Ok(())
}

/// Serializes catalogue writes with admissions, validation, and PB mutations.
/// Call inside the owning transaction; released when that transaction ends.
pub async fn lock_catalogue_write<C: sea_orm::ConnectionTrait>(
    database: &C,
) -> Result<(), sea_orm::DbErr> {
    database
        .query_one_raw(sea_orm::Statement::from_sql_and_values(
            sea_orm::DbBackend::Postgres,
            "SELECT pg_advisory_xact_lock($1)",
            [CATALOGUE_LOCK.into()],
        ))
        .await?;
    Ok(())
}

/// Connects to PostgreSQL.
pub async fn connect(
    settings: &DatabaseSettings,
    max_connections: u32,
) -> anyhow::Result<DatabaseConnection> {
    let mut options = ConnectOptions::new(settings.url.as_str());
    let statement_timeout_ms = settings
        .statement_timeout
        .duration()
        .as_millis()
        .to_string();
    options
        .max_connections(max_connections)
        .connect_timeout(settings.connect_timeout.duration())
        .acquire_timeout(settings.acquire_timeout.duration())
        .map_sqlx_postgres_opts(move |options| {
            options.options([("statement_timeout", statement_timeout_ms.as_str())])
        });
    let database = Database::connect(options)
        .await
        .context("failed to connect to PostgreSQL")?;
    Ok(database)
}

/// Applies all pending schema migrations.
pub async fn migrate(settings: &DatabaseSettings) -> anyhow::Result<()> {
    let database = connect(settings, 1).await?;
    MIGRATOR
        .run(database.get_postgres_connection_pool())
        .await
        .context("failed to migrate PostgreSQL")?;
    tracing::info!("database migrations are current");
    Ok(())
}

#[cfg(test)]
mod tests {
    #[sqlx::test(migrator = "super::MIGRATOR")]
    #[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
    async fn migrations_apply_to_fresh_database_and_can_run_again(
        pool: sqlx::PgPool,
    ) -> anyhow::Result<()> {
        // SQLx applied them once; check a second run is safe.
        super::MIGRATOR.run(&pool).await?;
        let applied: i64 =
            sqlx::query_scalar("SELECT count(*) FROM _sqlx_migrations WHERE success")
                .fetch_one(&pool)
                .await?;
        assert_eq!(usize::try_from(applied)?, super::MIGRATOR.iter().count());
        let jobs: Option<String> = sqlx::query_scalar("SELECT to_regclass('jobs')::text")
            .fetch_one(&pool)
            .await?;
        assert!(jobs.is_none());
        Ok(())
    }
}
