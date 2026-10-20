use super::*;
use crate::services::catalogue;

#[test]
fn bundled_badges_validate_and_reject_rules_and_missing_standard_variant() {
    let paths = std::fs::read_dir("assets/badges")
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "yaml"))
        .collect::<Vec<_>>();
    assert!(!paths.is_empty());
    catalogue::load::files(&paths).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let file = directory.path().join("badge.yaml");
    for invalid in [
        "variants: { gold: {} }",
        "variants: { standard: {} }\nrules: { type: podium }",
        "variants: { standard: { image: missing.svg } }",
        "variants: { standard: {}, 'BAD': {} }",
    ] {
        std::fs::write(&file, format!("kind: Badge\nid: anniversary\nname: Anniversary\ndescription: A special award\n{invalid}\n")).unwrap();
        assert!(catalogue::load::files(std::slice::from_ref(&file)).is_err());
    }
}

#[sqlx::test]
#[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
async fn presentation_seeding_is_idempotent_dry_runs_preserve_images_and_awards_resolve_live(
    pool: sqlx::PgPool,
) -> anyhow::Result<()> {
    let database = sea_orm::SqlxPostgresConnector::from_sqlx_postgres_pool(pool.clone());
    let store = object_store::memory::InMemory::new();
    let directory = tempfile::tempdir()?;
    let file = directory.path().join("badge.yaml");
    std::fs::write(directory.path().join("image.svg"), "<svg/>")?;
    std::fs::write(
        &file,
        "kind: Badge\nid: anniversary\nname: Anniversary\ndescription: A special award\nvariants:\n  standard:\n    image: image.svg\n",
    )?;
    let documents = catalogue::load::files(std::slice::from_ref(&file))?;
    assert_eq!(
        catalogue::seed(&database, &documents, None, true, |_| Ok(()))
            .await?
            .badges,
        ["anniversary"]
    );
    assert!(
        definition::Entity::find_by_id("anniversary")
            .one(&database)
            .await?
            .is_none()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM attachment")
            .fetch_one(&pool)
            .await?,
        0
    );
    catalogue::seed(&database, &documents, Some(&store), false, |_| Ok(())).await?;
    assert!(
        catalogue::seed(&database, &documents, Some(&store), false, |_| Ok(()))
            .await?
            .badges
            .is_empty()
    );
    let player: i64 = sqlx::query_scalar("INSERT INTO player (lfs_username, display_name) VALUES ('anniversary', 'Anniversary') RETURNING id").fetch_one(&pool).await?;
    crate::models::badge::grant_manual(&database, player, "anniversary", "standard", None).await?;
    let original = crate::models::badge::Model::resolved(&database, None, vec![player], true)
        .await?
        .remove(0);
    assert!(
        original
            .image
            .as_deref()
            .unwrap()
            .starts_with("/api/v1/attachments/")
    );
    // Definition edits become visible without rebuilding or regranting.
    let changed = std::fs::read_to_string(&file)?.replace("name: Anniversary", "name: Celebration");
    std::fs::write(&file, changed)?;
    let documents = catalogue::load::files(std::slice::from_ref(&file))?;
    catalogue::seed(&database, &documents, Some(&store), false, |_| Ok(())).await?;
    let updated = crate::models::badge::Model::resolved(&database, None, vec![player], true)
        .await?
        .remove(0);
    assert_eq!(updated.name, "Celebration");
    assert_eq!(original.image, updated.image);
    assert_eq!(original.id, updated.id);
    // Omitted image clears it, but dry runs cannot commit that change.
    std::fs::write(
        &file,
        "kind: Badge\nid: anniversary\nname: Celebration\ndescription: A special award\nvariants: { standard: {} }\n",
    )?;
    let documents = catalogue::load::files(std::slice::from_ref(&file))?;
    catalogue::seed(&database, &documents, None, true, |_| Ok(())).await?;
    let preserved = crate::models::badge::Model::resolved(&database, None, vec![player], true)
        .await?
        .remove(0);
    assert_eq!(original.image, preserved.image);
    Ok(())
}
