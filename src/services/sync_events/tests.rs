use super::*;
use crate::models::event::source::Change;
use sea_orm::SqlxPostgresConnector;
use time::{Duration, macros::datetime};

fn snapshot() -> Report {
    Report {
        leagues: vec![League {
            external_id: 432,
            name: "Fragmaster".into(),
            logo_url: None,
            source_url: "https://www.lfs.net/leagues/432".into(),
        }],
        seasons: vec![Season {
            external_id: 1347,
            league_id: 432,
            name: "2026".into(),
            source_url: "https://www.lfs.net/leagues/432/season/1347".into(),
        }],
        events: vec![Event {
            external_id: 6696,
            league_id: 432,
            season_id: 1347,
            title: "Thursday race".into(),
            starts_at: datetime!(2030-01-10 18:30 UTC),
            source_url: "https://www.lfs.net/leagues/432/season/1347/round/6696".into(),
            description: Some("Join us".into()),
            signup_url: None,
            track_label: Some("Blackwood GP".into()),
            cars_label: Some("XFG".into()),
        }],
        failures: Vec::new(),
    }
}

#[sqlx::test]
#[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
async fn events_import_is_idempotent_preserves_hidden_and_dry_runs_rollback(
    pool: sqlx::PgPool,
) -> anyhow::Result<()> {
    let database = SqlxPostgresConnector::from_sqlx_postgres_pool(pool.clone());
    let now = datetime!(2030-01-01 12:00 UTC);
    let dry = persist::apply(&database, snapshot(), true, now).await?;
    assert!(dry.failures.is_empty(), "{:?}", dry.failures);
    assert!(matches!(dry.events[0].change, Change::Created));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM league")
            .fetch_one(&pool)
            .await?,
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM event WHERE source = 'lfs_net'")
            .fetch_one(&pool)
            .await?,
        0
    );

    let applied = persist::apply(&database, snapshot(), false, now).await?;
    assert!(applied.failures.is_empty(), "{:?}", applied.failures);
    let id = applied.events[0].local_id;
    sqlx::query("UPDATE event SET hidden = true WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await?;
    let repeated = persist::apply(&database, snapshot(), false, now + Duration::hours(1)).await?;
    assert!(repeated.failures.is_empty(), "{:?}", repeated.failures);
    assert_eq!(repeated.events[0].local_id, id);
    assert!(matches!(repeated.events[0].change, Change::Unchanged));
    assert!(matches!(repeated.leagues[0].change, Change::Unchanged));
    assert!(matches!(repeated.seasons[0].change, Change::Unchanged));
    let stored: (bool, time::OffsetDateTime) =
        sqlx::query_as("SELECT hidden, updated_at FROM event WHERE id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await?;
    assert!(stored.0);
    assert_eq!(stored.1, now);

    let mut changed = snapshot();
    changed.events[0].title = "Rescheduled race".into();
    changed.events[0].starts_at = now - Duration::hours(1);
    let dry = persist::apply(&database, changed.clone(), true, now + Duration::hours(2)).await?;
    assert!(matches!(dry.events[0].change, Change::Updated));
    let title: String = sqlx::query_scalar("SELECT title FROM event WHERE id = $1")
        .bind(id)
        .fetch_one(&pool)
        .await?;
    assert_eq!(title, "Thursday race");
    let updated = persist::apply(&database, changed, false, now + Duration::hours(2)).await?;
    assert!(updated.failures.is_empty(), "{:?}", updated.failures);
    assert_eq!(updated.events[0].local_id, id);
    assert!(matches!(updated.events[0].change, Change::Updated));
    assert!(
        sqlx::query_scalar::<_, bool>("SELECT hidden FROM event WHERE id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await?
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM event")
            .fetch_one(&pool)
            .await?,
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, time::OffsetDateTime>(
            "SELECT last_synced_at FROM event WHERE source = 'lfs_net'"
        )
        .fetch_one(&pool)
        .await?,
        now + Duration::hours(2)
    );
    Ok(())
}

#[sqlx::test]
#[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
async fn events_import_retains_missing_records_and_rolls_back_failed_leagues(
    pool: sqlx::PgPool,
) -> anyhow::Result<()> {
    let database = SqlxPostgresConnector::from_sqlx_postgres_pool(pool.clone());
    let now = datetime!(2030-01-01 12:00 UTC);
    let first = persist::apply(&database, snapshot(), false, now).await?;
    assert!(first.failures.is_empty(), "{:?}", first.failures);
    let mut missing = snapshot();
    missing.events.clear();
    let result = persist::apply(&database, missing, false, now + Duration::hours(1)).await?;
    assert_eq!(result.stale_events, [first.events[0].local_id]);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM event WHERE NOT hidden")
            .fetch_one(&pool)
            .await?,
        1
    );

    let mut broken = snapshot();
    broken.leagues[0].name = "Must roll back".into();
    broken.events[0].title = " ".into();
    let mut other = snapshot();
    other.leagues[0].external_id = 999;
    other.seasons[0].league_id = 999;
    other.events[0].league_id = 999;
    broken.leagues.extend(other.leagues);
    broken.seasons.extend(other.seasons);
    broken.events.extend(other.events);
    broken
        .failures
        .push("league 123: source unavailable".into());
    let result = persist::apply(&database, broken, false, now + Duration::hours(2)).await?;
    assert_eq!(result.failures.len(), 2);
    assert_eq!(result.leagues.len(), 1);
    assert_eq!(result.events.len(), 1);
    assert_ne!(result.events[0].local_id, first.events[0].local_id);
    assert_eq!(result.events[0].external_id, "999/1347/6696");
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT name FROM league WHERE id = $1")
            .bind(first.leagues[0].local_id)
            .fetch_one(&pool)
            .await?,
        "Fragmaster"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM event")
            .fetch_one(&pool)
            .await?,
        2
    );
    assert_eq!(
        crate::models::league::Model::known_with_future_events(&database, now)
            .await?
            .len(),
        2
    );
    Ok(())
}

#[sqlx::test]
#[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
async fn events_parent_integrity_allows_one_offs_and_rejects_cross_league_seasons(
    pool: sqlx::PgPool,
) -> anyhow::Result<()> {
    let first: i64 = sqlx::query_scalar("INSERT INTO league (name) VALUES ('First') RETURNING id")
        .fetch_one(&pool)
        .await?;
    let second: i64 =
        sqlx::query_scalar("INSERT INTO league (name) VALUES ('Second') RETURNING id")
            .fetch_one(&pool)
            .await?;
    let season: i64 = sqlx::query_scalar(
        "INSERT INTO season (league_id, name) VALUES ($1, 'Season') RETURNING id",
    )
    .bind(first)
    .fetch_one(&pool)
    .await?;
    sqlx::query("INSERT INTO event (league_id, title, starts_at) VALUES ($1, 'One off', now())")
        .bind(first)
        .execute(&pool)
        .await?;
    assert!(sqlx::query("INSERT INTO event (league_id, season_id, title, starts_at) VALUES ($1, $2, 'Invalid', now())").bind(second).bind(season).execute(&pool).await.is_err());
    Ok(())
}

#[sqlx::test]
#[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
async fn events_sync_rejects_overlapping_runs(pool: sqlx::PgPool) -> anyhow::Result<()> {
    let database = SqlxPostgresConnector::from_sqlx_postgres_pool(pool);
    let connection = persist::lock(&database).await?;
    assert!(persist::lock(&database).await.is_err());
    drop(connection);
    // A query on a fresh connection gives the server time to process the closed session.
    let _replacement = persist::lock(&database).await?;
    Ok(())
}

#[sqlx::test]
#[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
async fn events_league_logos_reuse_bytes_preserve_cache_on_failure_and_do_not_upload_dry_runs(
    pool: sqlx::PgPool,
) -> anyhow::Result<()> {
    use crate::models::{attachment, league};
    use object_store::{ObjectStoreExt, path::Path};
    use sha2::{Digest, Sha256};
    let database = SqlxPostgresConnector::from_sqlx_postgres_pool(pool.clone());
    let now = datetime!(2030-01-01 12:00 UTC);
    let first = persist::apply(&database, snapshot(), false, now).await?;
    assert!(first.failures.is_empty());
    let id = first.leagues[0].local_id;
    let store = object_store::memory::InMemory::new();
    let logo = league::Logo {
        bytes: b"\x89PNG\r\n\x1a\nfirst".to_vec(),
        extension: "png",
        media_type: "image/png",
    };
    assert!(league::Model::cache_logo(&database, id, &logo, None, now).await?);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM attachment")
            .fetch_one(&pool)
            .await?,
        0
    );
    assert!(league::Model::cache_logo(&database, id, &logo, Some(&store), now).await?);
    let image_id: i64 = sqlx::query_scalar("SELECT image_attachment_id FROM league WHERE id = $1")
        .bind(id)
        .fetch_one(&pool)
        .await?;
    let attachment = attachment::Model::required(&database, image_id).await?;
    assert_eq!(
        store
            .get(&Path::from(attachment.object_key.as_str()))
            .await?
            .bytes()
            .await?
            .as_ref(),
        logo.bytes.as_slice()
    );
    assert!(
        !league::Model::cache_logo(&database, id, &logo, Some(&store), now + Duration::hours(1))
            .await?
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM attachment")
            .fetch_one(&pool)
            .await?,
        1
    );

    let changed = league::Logo {
        bytes: b"\x89PNG\r\n\x1a\nchanged".to_vec(),
        extension: "png",
        media_type: "image/png",
    };
    assert!(league::Model::cache_logo(&database, id, &changed, None, now).await?);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT image_attachment_id FROM league WHERE id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await?,
        image_id
    );
    // A file in the object's parent path forces a storage failure.
    let directory = tempfile::tempdir()?;
    let digest = hex::encode(Sha256::digest(&changed.bytes));
    std::fs::write(directory.path().join(&digest[..2]), "blocked")?;
    let failing = object_store::local::LocalFileSystem::new_with_prefix(directory.path())?;
    assert!(
        league::Model::cache_logo(&database, id, &changed, Some(&failing), now)
            .await
            .is_err()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT image_attachment_id FROM league WHERE id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await?,
        image_id
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM attachment")
            .fetch_one(&pool)
            .await?,
        1
    );

    let mut source = snapshot();
    source.leagues[0].logo_url = Some("https://evil.example/logo.png".into());
    let sources = source.leagues.clone();
    let mut report = persist::apply(&database, source, false, now).await?;
    images::sync(&database, Some(&store), &sources, &mut report).await;
    assert!(report.failures.is_empty());
    assert_eq!(report.warnings.len(), 1);
    assert_eq!(report.images_updated, 0);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT image_attachment_id FROM league WHERE id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await?,
        image_id
    );
    assert!(league::Model::cache_logo(&database, id, &changed, Some(&store), now).await?);
    assert_ne!(
        sqlx::query_scalar::<_, i64>("SELECT image_attachment_id FROM league WHERE id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await?,
        image_id
    );
    Ok(())
}

#[sqlx::test(migrations = false)]
#[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
async fn events_source_migration_preserves_records_and_uses_text_with_rust_enum(
    pool: sqlx::PgPool,
) -> anyhow::Result<()> {
    use crate::models::{event, league, season};
    use event::source::Source;
    use sea_orm::EntityTrait;
    let migrations = sqlx::migrate!();
    let before = sqlx::migrate::Migrator::with_migrations(
        migrations
            .iter()
            .filter(|migration| migration.version < 20261022140000)
            .cloned()
            .collect(),
    );
    before.run(&pool).await?;
    sqlx::raw_sql("INSERT INTO league (id, name) VALUES (10, 'Local'), (20, 'Imported');
        INSERT INTO season (id, league_id, name) VALUES (10, 10, 'Local season'), (20, 20, 'Imported season');
        INSERT INTO event (id, league_id, season_id, title, starts_at, hidden) VALUES
            (10, 10, 10, 'Local event', '2030-01-10T18:00:00Z', false),
            (20, 20, 20, 'Imported event', '2030-01-10T18:00:00Z', true);
        INSERT INTO league_source VALUES (20, 'lfs_net', '432', 'https://www.lfs.net/leagues/432', '2026-10-20T12:00:00Z', '2026-10-21T12:00:00Z');
        INSERT INTO season_source VALUES (20, 'lfs_net', '432/1347', 'https://www.lfs.net/leagues/432/season/1347', '2026-10-20T12:00:00Z', '2026-10-21T12:00:00Z');
        INSERT INTO event_source VALUES (20, 'lfs_net', '432/1347/6696', 'https://www.lfs.net/leagues/432/season/1347/round/6696', '2026-10-20T12:00:00Z', '2026-10-21T12:00:00Z');
        INSERT INTO attachment (object_key, media_type, extension) VALUES ('league-logo.png', 'image/png', 'png');
        UPDATE league SET image_attachment_id = (SELECT id FROM attachment WHERE object_key = 'league-logo.png') WHERE id = 20;")
        .execute(&pool).await?;
    migrations.run(&pool).await?;
    // Reapplying the migration set is harmless.
    migrations.run(&pool).await?;
    let database = SqlxPostgresConnector::from_sqlx_postgres_pool(pool.clone());
    let local = league::Entity::find_by_id(10)
        .one(&database)
        .await?
        .unwrap();
    assert_eq!(local.source, Source::Local);
    assert!(
        local.external_id.is_none()
            && local.source_url.is_none()
            && local.last_seen_at.is_none()
            && local.last_synced_at.is_none()
    );
    let imported = league::Entity::find_by_id(20)
        .one(&database)
        .await?
        .unwrap();
    assert_eq!(imported.source, Source::LfsNet);
    assert_eq!(imported.external_id.as_deref(), Some("432"));
    assert_eq!(
        imported.source_url.as_deref(),
        Some("https://www.lfs.net/leagues/432")
    );
    assert_eq!(imported.last_seen_at, Some(datetime!(2026-10-20 12:00 UTC)));
    assert_eq!(
        imported.last_synced_at,
        Some(datetime!(2026-10-21 12:00 UTC))
    );
    assert!(imported.image_attachment_id.is_some());
    let imported_season = season::Entity::find_by_id(20)
        .one(&database)
        .await?
        .unwrap();
    assert_eq!(imported_season.source, Source::LfsNet);
    assert_eq!(imported_season.league_id, 20);
    assert_eq!(imported_season.external_id.as_deref(), Some("432/1347"));
    let imported_event = event::Entity::find_by_id(20).one(&database).await?.unwrap();
    assert_eq!(imported_event.source, Source::LfsNet);
    assert_eq!(imported_event.league_id, 20);
    assert_eq!(imported_event.season_id, Some(20));
    assert!(imported_event.hidden);
    assert_eq!(imported_event.external_id.as_deref(), Some("432/1347/6696"));
    assert_eq!(imported_event.last_seen_at, imported.last_seen_at);
    assert_eq!(imported_event.last_synced_at, imported.last_synced_at);
    assert_eq!(
        event::Entity::find_by_id(10)
            .one(&database)
            .await?
            .unwrap()
            .source,
        Source::Local
    );
    assert_eq!(
        season::Entity::find_by_id(10)
            .one(&database)
            .await?
            .unwrap()
            .source,
        Source::Local
    );
    for table in ["league", "season", "event"] {
        let data_type: String = sqlx::query_scalar("SELECT data_type FROM information_schema.columns WHERE table_name = $1 AND column_name = 'source'").bind(table).fetch_one(&pool).await?;
        assert_eq!(data_type, "text");
        let mapping: Option<String> = sqlx::query_scalar("SELECT to_regclass($1)::text")
            .bind(format!("{table}_source"))
            .fetch_one(&pool)
            .await?;
        assert!(mapping.is_none());
    }
    // Local records can share a name and have no external identity.
    sqlx::query("INSERT INTO league (name) VALUES ('Local'), ('Local')")
        .execute(&pool)
        .await?;
    assert!(
        sqlx::query("INSERT INTO league (name, source) VALUES ('Invalid', 'unknown')")
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("INSERT INTO league (name, source) VALUES ('Missing key', 'lfs_net')")
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query(
            "INSERT INTO league (name, source, external_id) VALUES ('Duplicate', 'lfs_net', '432')"
        )
        .execute(&pool)
        .await
        .is_err()
    );
    assert!(
        sqlx::query(
            "INSERT INTO league (name, external_id) VALUES ('Local with remote ID', '432')"
        )
        .execute(&pool)
        .await
        .is_err()
    );
    // The importer finds the migrated IDs and leaves local rows untouched.
    let updated = persist::apply(
        &database,
        snapshot(),
        false,
        datetime!(2030-01-01 12:00 UTC),
    )
    .await?;
    assert!(updated.failures.is_empty(), "{:?}", updated.failures);
    assert_eq!(updated.leagues[0].local_id, 20);
    assert_eq!(updated.seasons[0].local_id, 20);
    assert_eq!(updated.events[0].local_id, 20);
    assert_eq!(
        league::Entity::find_by_id(10)
            .one(&database)
            .await?
            .unwrap()
            .name,
        "Local"
    );
    Ok(())
}
