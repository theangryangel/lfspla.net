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
    overlapping.id = "overlapping-era".parse().unwrap();
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
        MembershipEntity::find()
            .filter(crate::models::ranking_chart_membership::Column::EraId.eq(era.id))
            .count(&database)
            .await?,
        1
    );
    Ok(())
}

#[sqlx::test]
#[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
async fn shared_charts_keep_identity_until_last_membership_is_removed(
    pool: sqlx::PgPool,
) -> anyhow::Result<()> {
    use crate::models::{
        chart, era::definition::CombinationDefinition, ranking::RankingWithCharts,
    };
    let database = setup(pool.clone()).await?;
    let mut desired = definition::test_definitions().remove(0);
    let original_pair = desired.rankings[0].combinations.pairs().next().unwrap();
    desired.rankings[0].combinations = vec![original_pair.clone()].into();
    let mut shared = desired.rankings[0].clone();
    shared.id = "shared".to_owned().try_into().unwrap();
    desired.rankings.push(shared);
    apply(&database, &[desired.clone()], |_| Ok(())).await?;
    let era = Era::find_by_slug(&database, &desired.id).await?.unwrap();
    let chart = era.combinations().one(&database).await?.unwrap();
    assert_eq!(era.combinations().count(&database).await?, 1);
    let first = RankingWithCharts::find(&database, era.id, desired.rankings[0].id.as_str())
        .await?
        .unwrap();
    let shared = RankingWithCharts::find(&database, era.id, "shared")
        .await?
        .unwrap();
    assert_eq!(first.charts[0].id, shared.charts[0].id);

    sqlx::query(
        "INSERT INTO player (lfs_username, display_name) VALUES ('chart-test', 'Chart test')",
    )
    .execute(&pool)
    .await?;
    sqlx::query(
        "INSERT INTO hotlap (player_id, era_id, chart_id, track, vehicle, raw_vehicle_name,
        lap_time_ms, split_1_ms, split_2_ms, split_3_ms, split_4_ms, source, fingerprint,
        steering, player_flags, game_version, state)
        SELECT player.id, chart.era_id, chart.id, track_id, vehicle_id, vehicle_id,
        60000, 0, 0, 0, 0, 'lfsworld_v1', '2006-01-01:1', 'wheel', 0, '0.5A', 'valid'
        FROM player CROSS JOIN chart WHERE chart.id = $1",
    )
    .bind(chart.id)
    .execute(&pool)
    .await?;
    era.rebuild_personal_bests(&database).await?;
    let scores = first.definition.personal_scores(&database, 100).await?;
    assert_eq!(
        scores,
        shared.definition.personal_scores(&database, 100).await?
    );
    assert_eq!(scores.len(), 1);

    desired.rankings.remove(0);
    apply(&database, &[desired.clone()], |_| Ok(())).await?;
    assert_eq!(
        era.combinations().one(&database).await?.unwrap().id,
        chart.id
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM hotlap")
            .fetch_one(&pool)
            .await?,
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM hotlap_personal_best")
            .fetch_one(&pool)
            .await?,
        1
    );

    let replacement = CombinationDefinition {
        track: "BL1".parse()?,
        vehicle: "XRG".parse().unwrap(),
    };
    desired.rankings[0].combinations = vec![replacement].into();
    // A declined review cannot delete laps or chart identity.
    assert!(
        apply(&database, &[desired.clone()], |_| anyhow::bail!("declined"))
            .await
            .is_err()
    );
    assert!(
        chart::Entity::find_by_id(chart.id)
            .one(&database)
            .await?
            .is_some()
    );
    apply(&database, &[desired.clone()], |changes| {
        assert_eq!(changes[0].affected[0].laps, 1);
        Ok(())
    })
    .await?;
    assert!(
        chart::Entity::find_by_id(chart.id)
            .one(&database)
            .await?
            .is_none()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM hotlap")
            .fetch_one(&pool)
            .await?,
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM hotlap_personal_best")
            .fetch_one(&pool)
            .await?,
        0
    );

    desired.rankings[0].combinations = vec![original_pair].into();
    apply(&database, &[desired], |_| Ok(())).await?;
    assert_ne!(
        era.combinations().one(&database).await?.unwrap().id,
        chart.id
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM hotlap")
            .fetch_one(&pool)
            .await?,
        0
    );
    Ok(())
}
