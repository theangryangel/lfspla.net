use super::*;
use crate::services::catalogue::{self, CatalogueDefinition};
use sea_orm::SqlxPostgresConnector;

fn document(directory: &Path, name: &str, body: &str) -> PathBuf {
    let path = directory.join(name);
    std::fs::write(
        &path,
        format!("kind: CommunityCard\nid: test-card\ntitle: Test card\n{body}\n"),
    )
    .unwrap();
    path
}

#[test]
fn templates_validate_and_reject_unsafe_links_incomplete_content_and_unknown_fields() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(directory.path().join("logo.svg"), "<svg/>").unwrap();
    for content in [
        "theme: purple\nimage: logo.svg\ncontent: { type: partner, description: A community project, link: { label: Visit, url: 'https://example.com/' } }",
        "image: logo.svg\ncontent: { type: image, alt: Project banner, fit: contain }",
        "content: { type: announcement, body: New community features }",
        "image: logo.svg\ncontent: { type: partner, fit: cover, description: Project, link: { label: Visit, url: /hotlaps } }",
        "image: logo.svg\ncontent: { type: announcement, fit: contain, body: Community news }",
    ] {
        let path = document(directory.path(), "valid.yaml", content);
        let documents = catalogue::load::files(&[path]).unwrap();
        let CatalogueDefinition::CommunityCard(card) = &documents[0].definition else {
            panic!("wrong document kind")
        };
        let yaml = serde_saphyr::to_string(card).unwrap();
        let roundtrip: CommunityCardDefinition = serde_saphyr::from_str(&yaml).unwrap();
        assert_eq!(card.content, roundtrip.content);
    }
    for invalid in [
        "content: { type: links, links: [{ label: Charts, url: /hotlaps }] }",
        "layout: split\ncontent: { type: announcement, body: Hello }",
        "content: { type: image, alt: Banner }",
        "image: missing.png\ncontent: { type: image, alt: Banner }",
        "content: { type: partner, description: Project }",
        "content: { type: partner, fit: stretch, description: Project, link: { label: Visit, url: /hotlaps } }",
        "content: { type: announcement, body: ' ' }",
        "theme: arbitrary-css\ncontent: { type: announcement, body: Hello }",
        "content: { type: html, html: '<script/>' }",
        "content: { type: announcement, body: Hello, unknown: true }",
        "unknown: true\ncontent: { type: announcement, body: Hello }",
    ] {
        let path = document(directory.path(), "invalid.yaml", invalid);
        let error = catalogue::load::files(&[path])
            .err()
            .expect("invalid card accepted");
        assert!(format!("{error:#}").contains("invalid.yaml"));
    }
    for url in [
        "javascript:alert(1)",
        "data:text/html,hello",
        "//example.com/",
        "/\\example.com",
        "https://user:pass@example.com",
        "relative/path",
    ] {
        let path = document(
            directory.path(),
            "unsafe.yaml",
            &format!(
                "content: {{ type: partner, description: Project, link: {{ label: Visit, url: '{url}' }} }}"
            ),
        );
        assert!(catalogue::load::files(&[path]).is_err(), "accepted {url}");
    }
    let first = document(
        directory.path(),
        "first.yaml",
        "content: { type: announcement, body: Hello }",
    );
    let second = document(
        directory.path(),
        "second.yaml",
        "content: { type: announcement, body: Hello }",
    );
    assert!(
        catalogue::load::files(&[first, second])
            .err()
            .unwrap()
            .to_string()
            .contains("duplicate community card")
    );
}

#[test]
fn bundled_community_cards_validate() {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/community");
    let paths = [directory.clone(), directory.join("preview")]
        .into_iter()
        .flat_map(|directory| std::fs::read_dir(directory).unwrap())
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "yaml")
        })
        .collect::<Vec<_>>();
    assert!(!paths.is_empty());
    assert!(catalogue::load::files(&paths).is_ok());
}

#[test]
fn schedules_require_explicit_timezones_and_an_end_after_the_start() {
    let directory = tempfile::tempdir().unwrap();
    for schedule in [
        "",
        "starts_at: '2026-11-01T00:00:00Z'",
        "ends_at: '2026-12-01T00:00:00+00:00'",
        "enabled: false\nsequence: 10\nstarts_at: '2026-11-01T01:00:00+01:00'\nends_at: '2026-12-01T00:00:00Z'",
    ] {
        let path = document(
            directory.path(),
            "scheduled.yaml",
            &format!("{schedule}\ncontent: {{ type: announcement, body: Scheduled card }}"),
        );
        let documents = catalogue::load::files(&[path]).unwrap();
        let CatalogueDefinition::CommunityCard(card) = &documents[0].definition else {
            panic!("wrong document kind")
        };
        let yaml = serde_saphyr::to_string(card).unwrap();
        let roundtrip: CommunityCardDefinition = serde_saphyr::from_str(&yaml).unwrap();
        assert_eq!(card.starts_at, roundtrip.starts_at);
        assert_eq!(card.ends_at, roundtrip.ends_at);
        assert_eq!(card.enabled, roundtrip.enabled);
        assert_eq!(card.sequence, roundtrip.sequence);
    }
    for schedule in [
        "starts_at: '2026-11-02T00:00:00Z'\nends_at: '2026-11-01T00:00:00Z'",
        "starts_at: '2026-11-01T00:00:00Z'\nends_at: '2026-11-01T01:00:00+01:00'",
        "starts_at: '2026-11-01'",
        "starts_at: '2026-11-01T00:00:00'",
        "ends_at: 'not-a-date'",
    ] {
        let path = document(
            directory.path(),
            "invalid-schedule.yaml",
            &format!("{schedule}\ncontent: {{ type: announcement, body: Scheduled card }}"),
        );
        let error = catalogue::load::files(&[path])
            .err()
            .expect("invalid schedule accepted");
        assert!(format!("{error:#}").contains("invalid-schedule.yaml"));
    }
}

#[sqlx::test]
#[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
async fn visibility_obeys_schedule_boundaries_and_the_enabled_override(
    pool: sqlx::PgPool,
) -> anyhow::Result<()> {
    let now = time::macros::datetime!(2026-11-01 00:00 UTC);
    let before = now - time::Duration::days(1);
    let after = now + time::Duration::days(1);
    for (id, enabled, starts_at, ends_at, sequence) in [
        ("always", true, None, None, 30),
        ("scheduled", true, Some(before), Some(after), 10),
        ("starting-now", true, Some(now), None, 20),
        ("until-later", true, None, Some(after), 25),
        ("ending-now", true, None, Some(now), 0),
        ("future", true, Some(after), None, 0),
        ("disabled", false, Some(before), Some(after), 0),
    ] {
        sqlx::query("INSERT INTO community_card (id, title, enabled, starts_at, ends_at, sequence, content) VALUES ($1, $1, $2, $3, $4, $5, '{\"type\":\"announcement\",\"body\":\"Scheduled card\"}')")
            .bind(id).bind(enabled).bind(starts_at).bind(ends_at).bind(sequence).execute(&pool).await?;
    }
    // The database also rejects invalid windows, independently of catalogue validation.
    assert!(
        sqlx::query("UPDATE community_card SET starts_at = $1, ends_at = $1 WHERE id = 'always'")
            .bind(now)
            .execute(&pool)
            .await
            .is_err()
    );
    let database = SqlxPostgresConnector::from_sqlx_postgres_pool(pool);
    let visible = community_card::Entity::visible_at(now)
        .all(&database)
        .await?;
    assert_eq!(
        visible
            .iter()
            .map(|card| card.id.as_str())
            .collect::<Vec<_>>(),
        ["scheduled", "starting-now", "until-later", "always"]
    );
    Ok(())
}

#[sqlx::test]
#[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
async fn seeding_is_idempotent_dry_runs_roll_back_and_omitted_documents_are_preserved(
    pool: sqlx::PgPool,
) -> anyhow::Result<()> {
    let database = SqlxPostgresConnector::from_sqlx_postgres_pool(pool.clone());
    let store = object_store::memory::InMemory::new();
    let directory = tempfile::tempdir()?;
    std::fs::write(directory.path().join("logo.svg"), "<svg/>")?;
    let path = document(
        directory.path(),
        "card.yaml",
        "image: logo.svg\ncontent: { type: image, alt: Banner }",
    );
    let documents = catalogue::load::files(std::slice::from_ref(&path))?;
    assert_eq!(
        catalogue::seed(&database, &documents, None, true, |_| Ok(()))
            .await?
            .community_cards,
        ["test-card"]
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM community_card")
            .fetch_one(&pool)
            .await?,
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM attachment")
            .fetch_one(&pool)
            .await?,
        0
    );
    catalogue::seed(&database, &documents, Some(&store), false, |_| Ok(())).await?;
    let original = community_card::Entity::find_by_id("test-card")
        .one(&database)
        .await?
        .unwrap();
    assert!(original.image_attachment_id.is_some());
    assert!(
        catalogue::seed(&database, &documents, Some(&store), false, |_| Ok(()))
            .await?
            .community_cards
            .is_empty()
    );
    assert_eq!(
        community_card::Entity::find_by_id("test-card")
            .one(&database)
            .await?
            .unwrap(),
        original
    );
    // Replacing artwork reports a dry-run change without changing its identity.
    std::fs::write(directory.path().join("logo.svg"), "<svg><rect/></svg>")?;
    assert_eq!(
        catalogue::seed(&database, &documents, None, true, |_| Ok(()))
            .await?
            .community_cards,
        ["test-card"]
    );
    assert_eq!(
        community_card::Entity::find_by_id("test-card")
            .one(&database)
            .await?
            .unwrap(),
        original
    );
    catalogue::seed(&database, &documents, Some(&store), false, |_| Ok(())).await?;
    assert_ne!(
        community_card::Entity::find_by_id("test-card")
            .one(&database)
            .await?
            .unwrap()
            .image_attachment_id,
        original.image_attachment_id
    );
    document(
        directory.path(),
        "card.yaml",
        "enabled: false\ntheme: cyan\ncontent: { type: announcement, body: Updated }",
    );
    let documents = catalogue::load::files(&[path])?;
    catalogue::seed(&database, &documents, None, false, |_| Ok(())).await?;
    let updated = community_card::Entity::find_by_id("test-card")
        .one(&database)
        .await?
        .unwrap();
    assert!(!updated.enabled);
    assert_eq!(updated.theme, "cyan");
    assert!(updated.image_attachment_id.is_none());
    catalogue::seed(&database, &[], None, false, |_| Ok(())).await?;
    assert_eq!(
        community_card::Entity::find_by_id("test-card")
            .one(&database)
            .await?
            .unwrap(),
        updated
    );
    Ok(())
}

#[sqlx::test(migrations = false)]
#[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
async fn presentation_migration_removes_retired_cards_and_preserves_other_content(
    pool: sqlx::PgPool,
) -> anyhow::Result<()> {
    let migrations = sqlx::migrate!();
    let before = sqlx::migrate::Migrator::with_migrations(
        migrations
            .iter()
            .filter(|migration| migration.version < 20261019130000)
            .cloned()
            .collect(),
    );
    before.run(&pool).await?;
    sqlx::raw_sql(
        r#"
        INSERT INTO community_card (id, title, content) VALUES
        ('bookmarks', 'Bookmarks', '{"type":"links","links":[{"label":"Tracks","url":"/tracks"}]}'),
        ('keep', 'Community update', '{"type":"announcement","body":"Keep this card"}');
    "#,
    )
    .execute(&pool)
    .await?;
    migrations.run(&pool).await?;
    let database = SqlxPostgresConnector::from_sqlx_postgres_pool(pool);
    let rows = community_card::Entity::find().all(&database).await?;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, "keep");
    assert_eq!(rows[0].content["body"], "Keep this card");
    Ok(())
}
