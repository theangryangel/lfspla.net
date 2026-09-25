use super::*;
use sea_orm::SqlxPostgresConnector;

fn processor(pool: sqlx::PgPool) -> HotlapValidation {
    HotlapValidation {
        database: SqlxPostgresConnector::from_sqlx_postgres_pool(pool),
        object_store: Arc::new(object_store::memory::InMemory::new()),
        runtime: LfsRuntimeSettings::default(),
        settings: HlvcSettings::default(),
    }
}

#[sqlx::test]
#[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
async fn locked_rows_are_skipped_and_rollback_makes_them_available(
    pool: sqlx::PgPool,
) -> anyhow::Result<()> {
    sqlx::raw_sql(include_str!("fixtures.sql"))
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

#[sqlx::test]
#[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
async fn retry_delay_and_exhaustion_leave_pending_without_a_verdict(
    pool: sqlx::PgPool,
) -> anyhow::Result<()> {
    sqlx::raw_sql(include_str!("fixtures.sql"))
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

#[sqlx::test(migrations = false)]
#[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
async fn cutover_preserves_verdicts_resets_work_and_deletes_unknown_vehicles(
    pool: sqlx::PgPool,
) -> anyhow::Result<()> {
    let all = sqlx::migrate!();
    let before = sqlx::migrate::Migrator::with_migrations(
        all.iter()
            .filter(|m| m.version < 20260923120000)
            .cloned()
            .collect(),
    );
    before.run(&pool).await?;
    sqlx::raw_sql(include_str!("fixtures.sql"))
        .execute(&pool)
        .await?;
    sqlx::raw_sql(
        r"
INSERT INTO hotlap (
    player_id, era_id, track, vehicle, raw_vehicle_name, lap_time_ms,
    split_1_ms, split_2_ms, split_3_ms, split_4_ms, original_filename,
    spr_object_key, source, fingerprint, steering, abs_enabled, player_flags,
    game_version, state, attempt_count, hlvc_result_code, finished_at
)
SELECT player_id, era_id, track,
    CASE WHEN states.state = 'awaiting_vehicle' THEN NULL ELSE vehicle END,
    raw_vehicle_name, lap_time_ms, split_1_ms, split_2_ms, split_3_ms, split_4_ms,
    original_filename, spr_object_key, source, repeat(states.fingerprint, 64),
    steering, abs_enabled, player_flags, game_version, states.state,
    CASE WHEN states.state = 'awaiting_vehicle' THEN 0 ELSE 3 END,
    CASE WHEN states.state = 'valid' THEN 1 ELSE NULL END,
    CASE WHEN states.state = 'valid' THEN now() ELSE NULL END
FROM hotlap CROSS JOIN (VALUES
    ('validating', 'b'), ('error', 'c'), ('awaiting_vehicle', 'd'),
    ('valid', 'e'), ('invalid', 'f')
) AS states(state, fingerprint);
",
    )
    .execute(&pool)
    .await?;
    all.run(&pool).await?;
    let states: Vec<String> = sqlx::query_scalar("SELECT state FROM hotlap ORDER BY fingerprint")
        .fetch_all(&pool)
        .await?;
    assert_eq!(
        states,
        ["pending", "pending", "pending", "valid", "invalid"]
    );
    let reset: bool = sqlx::query_scalar("SELECT bool_and(attempt_count = 0 AND next_attempt_at IS NULL AND hlvc_result_code IS NULL) FROM hotlap WHERE state = 'pending'")
        .fetch_one(&pool).await?;
    assert!(reset);
    let jobs: Option<String> = sqlx::query_scalar("SELECT to_regclass('jobs')::text")
        .fetch_one(&pool)
        .await?;
    assert!(jobs.is_none());
    assert!(
        sqlx::query("UPDATE hotlap SET state = 'validating'")
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("UPDATE hotlap SET vehicle = NULL")
            .execute(&pool)
            .await
            .is_err()
    );
    Ok(())
}
