//! Shared process db.
//!
//! Shared database connection, migration, and catalogue locking operations.

pub(crate) mod escaped_like;

use crate::settings::DatabaseSettings;
use anyhow::Context;
use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use sqlx::migrate::Migrator;
static MIGRATOR: Migrator = sqlx::migrate!();

/// Serializes catalogue changes with admission and validation decisions.
pub(crate) const CATALOGUE_LOCK: i64 = 7_104_152_026;

/// Allows concurrent admission decisions while excluding catalogue rebuilds.
/// Call inside the owning transaction; released when that transaction ends.
pub(crate) async fn lock_admission<C: sea_orm::ConnectionTrait>(
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

/// Excludes admissions and PB mutations during bulk repairs.
/// Call inside the owning transaction; released when that transaction ends.
pub(crate) async fn lock_rebuild<C: sea_orm::ConnectionTrait>(
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

    #[sqlx::test(migrations = false)]
    #[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
    async fn chart_migration_deduplicates_memberships_preserves_positions_and_prunes_unselected_laps(
        pool: sqlx::PgPool,
    ) -> anyhow::Result<()> {
        let before = sqlx::migrate::Migrator::with_migrations(
            super::MIGRATOR
                .iter()
                .filter(|migration| migration.version < 20261005120000)
                .cloned()
                .collect(),
        );
        before.run(&pool).await?;
        sqlx::raw_sql(include_str!(
            "../services/validate_hotlap/fixtures_legacy.sql"
        ))
        .execute(&pool)
        .await?;
        sqlx::raw_sql("INSERT INTO ranking (era_id, slug, position, title, description,
                benchmark_percent, nation_max_points, nation_driver_limit)
            SELECT era.id, rankings.slug, position, rankings.slug, 'Test chart', 103, 10, 3
            FROM era CROSS JOIN (VALUES ('first', 0), ('second', 1)) AS rankings(slug, position);
            INSERT INTO ranking_chart (era_id, ranking_id, position, track_id, vehicle_id)
            SELECT era_id, id, 0, 'BL1', 'XFG' FROM ranking;
            UPDATE hotlap SET state = 'valid', hlvc_result_code = 1, finished_at = now();
            INSERT INTO hotlap_personal_best (era_id, player_id, track, vehicle, hotlap_id, position)
            SELECT era_id, player_id, track, vehicle, id, 1 FROM hotlap;
            INSERT INTO vehicle (id, kind, name, normalized_name, license)
            VALUES ('XRG', 'standard', 'XR GT', 'xr gt', 'demo') ON CONFLICT DO NOTHING;
            INSERT INTO hotlap (player_id, era_id, track, vehicle, raw_vehicle_name, lap_time_ms,
                split_1_ms, split_2_ms, split_3_ms, split_4_ms, original_filename, spr_object_key,
                source, fingerprint, steering, player_flags, game_version, state)
            SELECT player_id, era_id, track, 'XRG', 'XR GT', lap_time_ms, 0,0,0,0,
                'orphan.spr', 'orphan.spr', source, repeat('b',64), steering, player_flags, game_version, 'pending'
            FROM hotlap;").execute(&pool).await?;
        let original: Vec<(i64, i64)> =
            sqlx::query_as("SELECT hotlap_id, position FROM hotlap_personal_best")
                .fetch_all(&pool)
                .await?;
        super::MIGRATOR.run(&pool).await?;
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM chart")
                .fetch_one(&pool)
                .await?,
            1
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM ranking_chart_membership")
                .fetch_one(&pool)
                .await?,
            2
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM hotlap")
                .fetch_one(&pool)
                .await?,
            1
        );
        assert_eq!(
            original,
            sqlx::query_as::<_, (i64, i64)>("SELECT hotlap_id, position FROM hotlap_personal_best")
                .fetch_all(&pool)
                .await?
        );
        let consistent: bool = sqlx::query_scalar(
            "SELECT bool_and(hotlap.chart_id = pb.chart_id)
            FROM hotlap JOIN hotlap_personal_best pb ON pb.hotlap_id = hotlap.id",
        )
        .fetch_one(&pool)
        .await?;
        assert!(consistent);
        Ok(())
    }
}
