use super::*;
use crate::{
    models::{Vehicle as VehicleRecord, attachment::Model as Attachment},
    services::catalogue,
    vehicle_id::VehicleId,
};
use sea_orm::{SqlxPostgresConnector, TransactionTrait};

#[sqlx::test]
#[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
async fn imports_preserve_existing_images_and_omissions_without_touching_mods(
    pool: sqlx::PgPool,
) -> anyhow::Result<()> {
    let database = SqlxPostgresConnector::from_sqlx_postgres_pool(pool.clone());
    let store = object_store::memory::InMemory::new();
    let metadata_directory = tempfile::tempdir()?;
    let metadata_yaml = metadata_directory.path().join("XFG.yaml");
    std::fs::write(&metadata_yaml, "kind: Vehicle\nid: XFG\n")?;
    let metadata = catalogue::load::files(&[metadata_yaml])?;
    let preview = catalogue::seed(&database, &metadata, None, true, |_| Ok(())).await?;
    assert_eq!(preview.vehicles, ["XFG"]);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM vehicle")
            .fetch_one(&pool)
            .await?,
        0
    );
    let created = catalogue::seed(&database, &metadata, None, false, |_| Ok(())).await?;
    assert_eq!(created.vehicles, ["XFG"]);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM vehicle")
            .fetch_one(&pool)
            .await?,
        1
    );
    assert!(
        catalogue::seed(&database, &metadata, None, false, |_| Ok(()))
            .await?
            .vehicles
            .is_empty()
    );
    crate::services::catalogue::fixtures::vehicles(&database).await?;
    sqlx::query(
        "UPDATE vehicle SET name = 'Preserve XRG metadata', sequence = 999 WHERE id = 'XRG'",
    )
    .execute(&pool)
    .await?;
    VehicleRecord::refresh(
        &database,
        &[lfsplanet_lfs_api::RemoteVehicleMod {
            id: "ABCDEF".to_owned(),
            name: "Test mod".to_owned(),
            version: 7,
            class: None,
            author_username: Some("author".to_owned()),
            work_in_progress: false,
            published_at: None,
            is_private: false,
            is_tweak_mod: false,
            cover_thumb_url: None,
            metadata: serde_json::json!({"preserve": true}),
        }],
    )
    .await?;
    let mod_before = Entity::find_by_id("ABCDEF".parse::<VehicleId>()?)
        .one(&database)
        .await?
        .unwrap();
    let directory = tempfile::tempdir()?;
    let yaml = directory.path().join("XFG.yaml");
    let file = directory.path().join("cover.png");
    let artwork = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/vehicles");
    let xfg_bytes = std::fs::read(artwork.join("XFG.png"))?;
    let xrg_bytes = std::fs::read(artwork.join("XRG.png"))?;
    // Existing images use the same storage path as the retired importer.
    let transaction = database.begin().await?;
    for (id, bytes) in [("XFG", xfg_bytes.clone()), ("XRG", xrg_bytes.clone())] {
        let attachment = Attachment::store(
            &transaction,
            &store,
            bytes,
            "png",
            "image/png",
            Some(&format!("{id}.png")),
        )
        .await?;
        Entity::update_many()
            .filter(Column::Id.eq(id))
            .col_expr(Column::ImageAttachmentId, Expr::value(attachment.id))
            .col_expr(
                Column::ImageFetchedAt,
                Expr::value(time::OffsetDateTime::now_utc()),
            )
            .exec(&transaction)
            .await?;
    }
    transaction.commit().await?;
    let xfg_id = VehicleId(Vehicle::Xfg);
    let xrg_id = VehicleId(Vehicle::Xrg);
    let original = Entity::find_by_id(xfg_id).one(&database).await?.unwrap();
    let omitted = Entity::find_by_id(xrg_id).one(&database).await?.unwrap();
    std::fs::write(&file, xfg_bytes)?;
    std::fs::write(&yaml, "kind: Vehicle\nid: XFG\nimage: ./cover.png\n")?;
    let documents = catalogue::load::files(std::slice::from_ref(&yaml))?;
    assert!(
        catalogue::seed(&database, &documents, Some(&store), false, |_| Ok(()))
            .await?
            .vehicles
            .is_empty()
    );
    assert_eq!(
        Entity::find_by_id(xfg_id).one(&database).await?.unwrap(),
        original
    );
    // Different bytes are reported by dry-run without altering references or timestamps.
    std::fs::write(&file, xrg_bytes)?;
    assert_eq!(
        catalogue::seed(&database, &documents, None, true, |_| Ok(()))
            .await?
            .vehicles,
        ["XFG"]
    );
    assert_eq!(
        Entity::find_by_id(xfg_id).one(&database).await?.unwrap(),
        original
    );
    let attachments: i64 = sqlx::query_scalar("SELECT count(*) FROM attachment")
        .fetch_one(&pool)
        .await?;
    assert_eq!(attachments, 2);
    assert_eq!(
        catalogue::seed(&database, &documents, Some(&store), false, |_| Ok(()))
            .await?
            .vehicles,
        ["XFG"]
    );
    let changed = Entity::find_by_id(xfg_id).one(&database).await?.unwrap();
    assert_ne!(changed.image_attachment_id, original.image_attachment_id);
    assert!(changed.image_fetched_at.is_some());
    assert!(
        catalogue::seed(&database, &documents, Some(&store), false, |_| Ok(()))
            .await?
            .vehicles
            .is_empty()
    );
    std::fs::write(&yaml, "kind: Vehicle\nid: XFG\n")?;
    let documents = catalogue::load::files(&[yaml])?;
    catalogue::seed(&database, &documents, Some(&store), false, |_| Ok(())).await?;
    assert_eq!(
        Entity::find_by_id(xfg_id).one(&database).await?.unwrap(),
        changed
    );
    assert_eq!(
        Entity::find_by_id(xrg_id).one(&database).await?.unwrap(),
        omitted
    );
    assert_eq!(
        Entity::find_by_id(mod_before.id)
            .one(&database)
            .await?
            .unwrap(),
        mod_before
    );
    let attachments: i64 = sqlx::query_scalar("SELECT count(*) FROM attachment")
        .fetch_one(&pool)
        .await?;
    assert_eq!(attachments, 3);
    Ok(())
}
