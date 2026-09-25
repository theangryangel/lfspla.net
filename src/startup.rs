//! Shared process startup.
//!
//! Shared database connection and migration operations.

use anyhow::Context;
use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use sqlx::migrate::Migrator;

use crate::settings::DatabaseSettings;

static MIGRATOR: Migrator = sqlx::migrate!();

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
