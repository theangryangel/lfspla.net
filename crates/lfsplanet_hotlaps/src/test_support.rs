//! Shared test data for hotlaps and the binary that composes it.

use std::path::PathBuf;

/// A player, era, chart, ranking, replay attachment and pending upload.
pub const FIXTURE_SQL: &str = include_str!("../fixtures/hotlaps.sql");

/// The workspace catalogue directory.
pub fn assets_dir() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets"))
}

/// Every bundled catalogue file in `directory` with the given extension.
pub fn bundled_files(directory: &str, extension: &str) -> Vec<PathBuf> {
    std::fs::read_dir(assets_dir().join(directory))
        .expect("bundled catalogue directory is readable")
        .map(|entry| entry.expect("bundled catalogue entry is readable").path())
        .filter(|path| path.extension().is_some_and(|found| found == extension))
        .collect()
}

/// Counts every row in `table`, which must be a fixed identifier.
pub async fn count(pool: &sqlx::PgPool, table: &'static str) -> sqlx::Result<i64> {
    // Only `'static` literal table names written in tests reach this query.
    sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT count(*) FROM {table}")))
        .fetch_one(pool)
        .await
}

/// Runs a deleting storage GC pass that treats every object as old enough.
pub async fn sweep(
    store: &dyn object_store::ObjectStore,
    database: &sea_orm::DatabaseConnection,
) -> anyhow::Result<lfsplanet_core::services::storage_gc::GcSummary> {
    lfsplanet_core::services::storage_gc::sweep(
        store,
        database,
        std::time::SystemTime::now() + std::time::Duration::from_secs(2),
        std::time::Duration::ZERO,
        true,
        |_| (),
    )
    .await
}
