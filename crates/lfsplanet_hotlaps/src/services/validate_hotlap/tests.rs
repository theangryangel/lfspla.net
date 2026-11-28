use super::*;
use sea_orm::SqlxPostgresConnector;
fn processor(pool: sqlx::PgPool) -> HotlapValidation {
    HotlapValidation {
        database: SqlxPostgresConnector::from_sqlx_postgres_pool(pool),
        object_store: Arc::new(object_store::memory::InMemory::new()),
        runtime: lfsplanet_lfs_installations::RuntimeConfig::default(),
        installation_root: "/srv/lfs".into(),
        settings: ValidationSettings::default(),
    }
}

#[sqlx::test(migrator = "lfsplanet_core::db::MIGRATOR")]
#[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
async fn locked_rows_are_skipped_and_rollback_makes_them_available(
    pool: sqlx::PgPool,
) -> anyhow::Result<()> {
    sqlx::raw_sql(crate::test_support::FIXTURE_SQL)
        .execute(&pool)
        .await?;
    let mut lock = pool.begin().await?;
    sqlx::query("SELECT id FROM hotlap FOR UPDATE")
        .fetch_one(&mut *lock)
        .await?;
    let processor = processor(pool.clone());
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(2), processor.process_next()).await??,
        Step::Idle
    );
    lock.rollback().await?;
    assert_eq!(processor.process_next().await?, Step::Processed);
    let (state, attempts, error): (String, i32, Option<String>) =
        sqlx::query_as("SELECT state, attempt_count, error_detail FROM hotlap")
            .fetch_one(&pool)
            .await?;
    assert_eq!(state, "pending");
    assert_eq!(attempts, 1);
    assert!(error.is_some());
    Ok(())
}

#[sqlx::test(migrator = "lfsplanet_core::db::MIGRATOR")]
#[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
async fn retry_delay_and_exhaustion_leave_pending_without_a_verdict(
    pool: sqlx::PgPool,
) -> anyhow::Result<()> {
    sqlx::raw_sql(crate::test_support::FIXTURE_SQL)
        .execute(&pool)
        .await?;
    let processor = processor(pool.clone());
    assert_eq!(processor.process_next().await?, Step::Processed);
    assert_eq!(processor.process_next().await?, Step::Idle);
    sqlx::query(
        "UPDATE hotlap SET attempt_count = $1, next_attempt_at = now() - interval '1 second'",
    )
    .bind(MAX_ATTEMPTS - 1)
    .execute(&pool)
    .await?;
    assert_eq!(processor.process_next().await?, Step::Processed);
    assert_eq!(processor.process_next().await?, Step::Idle);
    let (state, attempts, code, retry): (String, i32, Option<i16>, Option<time::OffsetDateTime>) =
        sqlx::query_as(
            "SELECT state, attempt_count, hlvc_result_code, next_attempt_at FROM hotlap",
        )
        .fetch_one(&pool)
        .await?;
    assert_eq!(
        (state.as_str(), attempts, code, retry),
        ("pending", MAX_ATTEMPTS, None, None)
    );
    Ok(())
}

#[sqlx::test(migrator = "lfsplanet_core::db::MIGRATOR")]
#[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
async fn raf_names_use_hotlap_metadata_and_preserve_the_generated_name(
    pool: sqlx::PgPool,
) -> anyhow::Result<()> {
    sqlx::raw_sql(crate::test_support::FIXTURE_SQL)
        .execute(&pool)
        .await?;
    let processor = processor(pool);
    let transaction = processor.database.begin().await?;
    let hotlap = HotlapEntity::find().one(&transaction).await?.unwrap();
    let file = processor
        .store_raf(
            &transaction,
            &hotlap,
            lfsplanet_lfs_installations::RafFile {
                filename: "actual-LFS-output.RAF".to_owned(),
                bytes: b"telemetry".to_vec(),
            },
        )
        .await?;
    assert_eq!(
        file.original_filename.as_deref(),
        Some("actual-LFS-output.RAF")
    );
    assert_eq!(
        file.filename.as_deref(),
        Some("worker-test_BL1_XFG_100000.raf")
    );
    transaction.commit().await?;
    Ok(())
}
