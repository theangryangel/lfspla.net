use super::*;
use crate::services::{catalogue, storage_gc};
use sea_orm::SqlxPostgresConnector;
use std::time::{Duration, SystemTime};

#[sqlx::test]
#[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
async fn moving_a_nested_track_updates_its_location_and_preserves_references(
    pool: sqlx::PgPool,
) -> anyhow::Result<()> {
    sqlx::raw_sql(include_str!("../../validate_hotlap/fixtures.sql"))
        .execute(&pool)
        .await?;
    let database = SqlxPostgresConnector::from_sqlx_postgres_pool(pool.clone());
    let directory = tempfile::tempdir()?;
    let original_yaml = directory.path().join("BL.yaml");
    let destination_yaml = directory.path().join("custom.yaml");
    let store = object_store::memory::InMemory::new();
    std::fs::write(directory.path().join("BL1.svg"), "<svg/>")?;
    std::fs::write(
        &original_yaml,
        "kind: Location\nid: BL\nname: Blackwood\ntracks:\n  - id: BL1\n    outline: ./BL1.svg\n",
    )?;
    let documents = catalogue::load::files(std::slice::from_ref(&original_yaml))?;
    catalogue::seed(&database, &documents, Some(&store), false, |_| Ok(())).await?;
    let original = Entity::find_by_id(crate::track_id::TrackId(Track::Bl1))
        .one(&database)
        .await?
        .unwrap();
    assert!(original.outline_attachment_id.is_some());

    std::fs::write(&original_yaml, "kind: Location\nid: BL\nname: Blackwood\n")?;
    std::fs::write(
        &destination_yaml,
        "kind: Location\nid: CUSTOM\nname: Custom grouping\ntracks:\n  - id: BL1\n",
    )?;
    let documents = catalogue::load::files(&[original_yaml, destination_yaml])?;
    let preview = catalogue::seed(&database, &documents, None, true, |_| Ok(())).await?;
    assert_eq!(preview.tracks, ["BL1"]);
    assert_eq!(
        Entity::find_by_id(original.id)
            .one(&database)
            .await?
            .unwrap(),
        original
    );
    let applied = catalogue::seed(&database, &documents, None, false, |_| Ok(())).await?;
    assert_eq!(applied.tracks, ["BL1"]);
    let mut expected = original;
    expected.location = "CUSTOM".into();
    assert_eq!(
        Entity::find_by_id(expected.id)
            .one(&database)
            .await?
            .unwrap(),
        expected
    );
    // Moving a track keeps existing chart and hotlap identities intact.
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM hotlap JOIN chart ON chart.id = hotlap.chart_id \
             WHERE chart.track_id = 'BL1' AND hotlap.track = 'BL1'"
        )
        .fetch_one(&pool)
        .await?,
        1
    );
    assert!(
        catalogue::seed(&database, &documents, None, false, |_| Ok(()))
            .await?
            .tracks
            .is_empty()
    );
    Ok(())
}

#[sqlx::test]
#[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
async fn artwork_attachments_are_stable_and_gc_discovers_all_parent_references(
    pool: sqlx::PgPool,
) -> anyhow::Result<()> {
    let database = SqlxPostgresConnector::from_sqlx_postgres_pool(pool.clone());
    let store = object_store::memory::InMemory::new();
    let directory = tempfile::tempdir()?;
    let file = directory.path().join("BL1.svg");
    let yaml = directory.path().join("BL1.yaml");
    std::fs::write(&file, "<svg/>")?;
    std::fs::write(
        &yaml,
        "kind: Location\nid: BL\nname: Blackwood\ntracks:\n  - id: BL1\n    outline: ./BL1.svg\n",
    )?;
    let documents = catalogue::load::files(std::slice::from_ref(&yaml))?;
    let preview = catalogue::seed(&database, &documents, None, true, |_| Ok(())).await?;
    assert_eq!(preview.tracks, ["BL1"]);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM track")
            .fetch_one(&pool)
            .await?,
        0
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM attachment")
        .fetch_one(&pool)
        .await?;
    assert_eq!(count, 0);
    assert_eq!(
        catalogue::seed(&database, &documents, Some(&store), false, |_| Ok(()))
            .await?
            .tracks,
        ["BL1"]
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM track")
            .fetch_one(&pool)
            .await?,
        1
    );
    sqlx::query("INSERT INTO track (id, name, license, location, reverse, open_configuration, sequence) VALUES ('BL2', 'Preserve BL2 metadata', 'demo', 'BL', false, false, 999)")
        .execute(&pool).await?;
    let original = Entity::find_by_id(crate::track_id::TrackId(Track::Bl1))
        .one(&database)
        .await?
        .unwrap();
    assert!(
        catalogue::seed(&database, &documents, Some(&store), false, |_| Ok(()))
            .await?
            .tracks
            .is_empty()
    );
    assert_eq!(
        Entity::find_by_id(original.id)
            .one(&database)
            .await?
            .unwrap()
            .outline_attachment_id,
        original.outline_attachment_id
    );
    // Omitted fields preserve attachments too.
    std::fs::write(
        &yaml,
        "kind: Location\nid: BL\nname: Blackwood\ntracks:\n  - id: BL1\n",
    )?;
    let omitted = catalogue::load::files(&[yaml])?;
    catalogue::seed(&database, &omitted, Some(&store), false, |_| Ok(())).await?;
    assert_eq!(
        Entity::find_by_id(original.id)
            .one(&database)
            .await?
            .unwrap()
            .outline_attachment_id,
        original.outline_attachment_id
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT name FROM track WHERE id = 'BL2'")
            .fetch_one(&pool)
            .await?,
        "Preserve BL2 metadata"
    );
    let sweep = storage_gc::sweep(
        &store,
        &database,
        SystemTime::now() + Duration::from_secs(2),
        Duration::ZERO,
        true,
        |_| (),
    )
    .await?;
    assert_eq!(sweep.referenced, 1);
    // A new parent table needs no GC code changes, including quoted identifiers.
    sqlx::raw_sql(
        "CREATE TABLE \"other attachment parent\" (\"file id\" BIGINT REFERENCES attachment(id));",
    )
    .execute(&pool)
    .await?;
    sqlx::query("INSERT INTO \"other attachment parent\" VALUES ($1)")
        .bind(original.outline_attachment_id)
        .execute(&pool)
        .await?;
    sqlx::query("UPDATE track SET outline_attachment_id = NULL WHERE id = 'BL1'")
        .execute(&pool)
        .await?;
    let sweep = storage_gc::sweep(
        &store,
        &database,
        SystemTime::now() + Duration::from_secs(2),
        Duration::ZERO,
        true,
        |_| (),
    )
    .await?;
    assert_eq!(sweep.referenced, 1);
    sqlx::query("DELETE FROM \"other attachment parent\"")
        .execute(&pool)
        .await?;
    let sweep = storage_gc::sweep(
        &store,
        &database,
        SystemTime::now() + Duration::from_secs(2),
        Duration::ZERO,
        true,
        |_| (),
    )
    .await?;
    assert_eq!(sweep.eligible, 1);
    Ok(())
}
