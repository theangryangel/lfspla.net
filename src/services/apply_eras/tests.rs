use super::*;
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, SqlxPostgresConnector};

async fn setup(pool: sqlx::PgPool) -> anyhow::Result<DatabaseConnection> {
    let database = SqlxPostgresConnector::from_sqlx_postgres_pool(pool);
    crate::models::Track::sync(&database).await?;
    crate::models::Vehicle::sync_builtin(&database).await?;
    Ok(database)
}

#[sqlx::test]
#[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
async fn applies_complete_definitions_and_repeating_them_changes_nothing(
    pool: sqlx::PgPool,
) -> anyhow::Result<()> {
    let database = setup(pool).await?;
    let desired = definition::test_definitions().remove(0);
    let applied = apply(&database, std::slice::from_ref(&desired), |changes| {
        assert_eq!(changes.len(), 1);
        assert!(changes[0].existing.is_none());
        Ok(())
    })
    .await?;
    assert_eq!(applied, vec![desired.id.clone()]);
    assert_eq!(
        definition::find(&database, &desired.id).await?,
        Some(desired.clone())
    );
    assert!(apply(&database, &[desired], |_| Ok(())).await?.is_empty());
    Ok(())
}

#[sqlx::test]
#[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
async fn rejected_review_preserves_the_catalogue_and_releases_its_lock(
    pool: sqlx::PgPool,
) -> anyhow::Result<()> {
    let database = setup(pool).await?;
    let original = definition::test_definitions().remove(0);
    apply(&database, std::slice::from_ref(&original), |_| Ok(())).await?;
    let mut desired = original.clone();
    desired.title = "Changed physics".to_owned();
    desired.open = !desired.open;
    let refused = apply(&database, std::slice::from_ref(&desired), |changes| {
        assert_eq!(changes[0].existing.as_ref(), Some(&original));
        anyhow::bail!("review declined")
    })
    .await;
    assert!(refused.is_err());
    assert_eq!(
        definition::find(&database, &original.id).await?,
        Some(original)
    );
    let applied = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        apply(&database, &[desired], |_| Ok(())),
    )
    .await??;
    assert_eq!(applied.len(), 1);
    Ok(())
}

#[sqlx::test]
#[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
async fn overlapping_versions_roll_back_every_change_in_the_batch(
    pool: sqlx::PgPool,
) -> anyhow::Result<()> {
    let database = setup(pool).await?;
    let original = definition::test_definitions().remove(0);
    apply(&database, std::slice::from_ref(&original), |_| Ok(())).await?;
    let mut modified = original.clone();
    modified.title = "Must roll back".to_owned();
    let mut overlapping = original.clone();
    overlapping.id = "overlapping-era".to_owned();
    assert!(
        apply(&database, &[modified, overlapping], |_| Ok(()))
            .await
            .is_err()
    );
    assert_eq!(
        definition::find(&database, &original.id).await?,
        Some(original.clone())
    );
    assert_eq!(EraEntity::find().count(&database).await?, 1);
    let era = Era::find_by_slug(&database, &original.id).await?.unwrap();
    assert_eq!(
        RankingChartEntity::find()
            .filter(crate::models::ranking_chart::Column::EraId.eq(era.id))
            .count(&database)
            .await?,
        1
    );
    Ok(())
}
