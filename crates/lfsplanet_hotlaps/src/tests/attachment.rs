//! Attachment storage and garbage collection against hotlap references.

use lfsplanet_core::models::attachment::*;
use object_store::{ObjectStoreExt, path::Path};
use sea_orm::{DbBackend, Statement, entity::prelude::*};
use sea_orm::{SqlxPostgresConnector, TransactionTrait};
use std::{sync::Arc, time::Duration};

#[sqlx::test(migrator = "lfsplanet_core::db::MIGRATOR")]
#[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
async fn shared_files_survive_reference_deletion_and_gc_waits_for_uploads(
    pool: sqlx::PgPool,
) -> anyhow::Result<()> {
    sqlx::raw_sql(crate::test_support::FIXTURE_SQL)
        .execute(&pool)
        .await?;
    let database = SqlxPostgresConnector::from_sqlx_postgres_pool(pool.clone());
    let store = Arc::new(object_store::memory::InMemory::new());
    let transaction = database.begin().await?;
    let first = Model::store(
        &transaction,
        &*store,
        b"same bytes".to_vec(),
        "spr",
        "application/octet-stream",
        Some("lap.spr"),
    )
    .await?;
    transaction.commit().await?;
    let transaction = database.begin().await?;
    let second = Model::store(
        &transaction,
        &*store,
        b"same bytes".to_vec(),
        "spr",
        "application/octet-stream",
        Some("other.spr"),
    )
    .await?;
    assert_ne!(first.id, second.id);
    assert_ne!(first.public_id, second.public_id);
    assert_eq!(first.object_key, second.object_key);
    assert_eq!(first.filename.as_deref(), Some("lap.spr"));
    assert_eq!(second.filename.as_deref(), Some("other.spr"));
    assert_eq!(first.public_id.get_version_num(), 4);
    assert_eq!(
        first.object_key,
        object_key(first.sha256.as_deref().unwrap(), "spr")
    );
    transaction
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE hotlap SET spr_attachment_id = $1",
            [first.id.into()],
        ))
        .await?;
    transaction.execute_raw(Statement::from_string(DbBackend::Postgres,
        "INSERT INTO hotlap (player_id, era_id, chart_id, track, vehicle, raw_vehicle_name, lap_time_ms,
            split_1_ms, split_2_ms, split_3_ms, split_4_ms, spr_attachment_id,
            source, fingerprint, steering, player_flags, game_version, state)
         SELECT player_id, era_id, chart_id, track, vehicle, raw_vehicle_name, lap_time_ms,
            0,0,0,0, spr_attachment_id, source, repeat('b',64), steering, player_flags, game_version, state FROM hotlap".to_owned(),
    )).await?;
    transaction
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE hotlap SET spr_attachment_id = $1 WHERE fingerprint = repeat('b',64)",
            [second.id.into()],
        ))
        .await?;
    // GC must wait for this attachment's reference transaction to commit.
    let gc_database = database.clone();
    let gc_store = store.clone();
    let mut task =
        tokio::spawn(async move { crate::test_support::sweep(&*gc_store, &gc_database).await });
    assert!(
        tokio::time::timeout(Duration::from_millis(100), &mut task)
            .await
            .is_err()
    );
    transaction.commit().await?;
    assert_eq!(task.await??.referenced, 1);
    assert!(
        store
            .get(&Path::from(first.object_key.as_str()))
            .await
            .is_ok()
    );
    let (hotlap_id, player_id): (i64, i64) =
        sqlx::query_as("SELECT id, player_id FROM hotlap ORDER BY id LIMIT 1")
            .fetch_one(&pool)
            .await?;
    crate::models::Hotlap::delete_owned(&database, hotlap_id, player_id).await?;
    let retained = crate::test_support::sweep(&*store, &database).await?;
    assert_eq!(retained.referenced, 1);
    assert!(Entity::find_by_id(first.id).one(&database).await?.is_none());
    assert!(
        Entity::find_by_id(second.id)
            .one(&database)
            .await?
            .is_some()
    );
    assert!(
        store
            .get(&Path::from(first.object_key.as_str()))
            .await
            .is_ok()
    );
    sqlx::query("DELETE FROM hotlap").execute(&pool).await?;
    let summary = crate::test_support::sweep(&*store, &database).await?;
    assert_eq!(summary.eligible, 1);
    assert!(
        store
            .get(&Path::from(first.object_key.as_str()))
            .await
            .is_err()
    );
    assert!(Entity::find_by_id(first.id).one(&database).await?.is_none());
    Ok(())
}
