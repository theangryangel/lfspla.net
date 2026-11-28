use super::*;
use crate::models::ranking::{RankingBadgeDefinition, RankingBadgeQualification};
use lfsplanet_core::models::badge::*;
#[test]
fn completion_awards_do_not_expand_the_primary_award() {
    let definition: RankingBadgeDefinition = serde_json::from_value(serde_json::json!({
        "id": "nutter",
        "qualification": { "type": "ranked", "limit": 3 },
        "completion": { "id": "gabor" }
    }))
    .unwrap();
    assert_eq!(definition.result_limit(), u64::MAX);
    for (index, completed, primary, completion) in [
        (0, 18, true, true),
        (2, 17, true, false),
        (3, 18, false, true),
        (20, 17, false, false),
    ] {
        assert_eq!(
            qualifies_for_ranking_badge(definition.qualification, completed, 18, index),
            primary
        );
        assert_eq!(
            qualifies_for_ranking_badge(RankingBadgeQualification::Complete, completed, 18, index),
            completion
        );
    }
    let legacy: RankingBadgeDefinition = serde_json::from_value(serde_json::json!({
        "id": "nutter",
        "qualification": { "type": "ranked", "limit": 3 }
    }))
    .unwrap();
    assert_eq!(legacy.result_limit(), 3);
    assert!(legacy.completion.is_none());
}

#[sqlx::test(migrator = "lfsplanet_core::db::MIGRATOR")]
#[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
async fn manual_grants_survive_rebuilds_and_revoke_does_not_remove_automatic_awards(
    pool: sqlx::PgPool,
) -> anyhow::Result<()> {
    use sea_orm::{SqlxPostgresConnector, TransactionTrait};
    let database = SqlxPostgresConnector::from_sqlx_postgres_pool(pool.clone());
    crate::services::catalogue::fixtures::tracks(&database).await?;
    crate::services::catalogue::fixtures::vehicles(&database).await?;
    let desired = crate::models::era::definition::test_definitions().remove(0);
    let source = crate::services::catalogue::load::SourceDefinition {
        source: "fixture.yaml".into(),
        definition: crate::services::catalogue::CatalogueDefinition::Era(desired.clone()),
    };
    crate::services::catalogue::seed(&database, &[source], None, false, |_| Ok(())).await?;
    let era = crate::models::Era::find_by_slug(&database, &desired.id)
        .await?
        .unwrap();
    let player: i64 = sqlx::query_scalar("INSERT INTO player (lfs_username, display_name) VALUES ('badge-test', 'Badge Test') RETURNING id")
        .fetch_one(&pool).await?;
    grant_manual(&database, player, "mhr", "standard", None).await?;
    grant_manual(&database, player, "mhr", "gold", Some("A special award")).await?;
    assert_eq!(Model::list_manual(&database, player).await?.len(), 1);
    // Manual grants also appear for players without any hotlaps or era activity.
    let profile = lfsplanet_core::models::player::Entity::find_by_id(player)
        .one(&database)
        .await?
        .unwrap()
        .profile(&database)
        .await?;
    assert_eq!(profile.badges[0].description, "A special award");
    assert!(profile.eras.is_empty());
    let transaction = database.begin().await?;
    let awards = HashMap::from([(
        player,
        vec![CalculatedAward::new(
            "mhr",
            "silver",
            "ranking:fixture:primary",
            "Rank - #2".to_owned(),
        )],
    )]);
    Model::replace_era(&transaction, era.id, awards).await?;
    transaction.commit().await?;
    assert_eq!(
        Model::list_for_players(&database, era.id, [player]).await?[&player].len(),
        2
    );
    assert_eq!(revoke_manual(&database, player, "mhr").await?, 1);
    assert_eq!(
        Model::list_for_players(&database, era.id, [player]).await?[&player][0].variant,
        "silver"
    );
    grant_manual(&database, player, "mhr", "gold", None).await?;
    era.rebuild_badges(&database).await?;
    let awards = Model::list_for_players(&database, era.id, [player]).await?;
    assert_eq!(awards[&player].len(), 1);
    assert_eq!(awards[&player][0].variant, "gold");
    assert!(
        grant_manual(&database, player, "nonexistent", "standard", None)
            .await
            .is_err()
    );
    assert!(
        grant_manual(&database, player, "mhr", "nonexistent", None)
            .await
            .is_err()
    );
    Ok(())
}

#[sqlx::test(migrator = "lfsplanet_core::db::MIGRATOR")]
#[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
async fn automatic_badges_keep_ranking_completion_podium_and_newbie_eligibility(
    pool: sqlx::PgPool,
) -> anyhow::Result<()> {
    use sea_orm::TransactionTrait;

    let database = sea_orm::SqlxPostgresConnector::from_sqlx_postgres_pool(pool.clone());
    crate::services::catalogue::fixtures::tracks(&database).await?;
    crate::services::catalogue::fixtures::vehicles(&database).await?;
    let mut desired = crate::models::era::definition::test_definitions().remove(0);
    desired.rankings[0].badge = Some(serde_json::from_value(serde_json::json!({
        "id": "mhr", "qualification": {"type":"ranked", "limit":3}, "completion":{"id":"gabor"}
    }))?);
    let source = crate::services::catalogue::load::SourceDefinition {
        source: "fixture.yaml".into(),
        definition: crate::services::catalogue::CatalogueDefinition::Era(desired.clone()),
    };
    crate::services::catalogue::seed(&database, &[source], None, false, |_| Ok(())).await?;
    let era = crate::models::Era::find_by_slug(&database, &desired.id)
        .await?
        .unwrap();
    let mut players = Vec::new();
    for index in 1..=4 {
        let player: i64 = sqlx::query_scalar(
            "INSERT INTO player (lfs_username, display_name) VALUES ($1, $1) RETURNING id",
        )
        .bind(format!("badge-driver-{index}"))
        .fetch_one(&pool)
        .await?;
        players.push(player);
        sqlx::query(
            "INSERT INTO hotlap (player_id, era_id, chart_id, track, vehicle, raw_vehicle_name,
            lap_time_ms, split_1_ms, split_2_ms, split_3_ms, split_4_ms, source, fingerprint,
            steering, player_flags, game_version, state)
            SELECT $1, chart.era_id, chart.id, track_id, vehicle_id, vehicle_id,
            $3, 0, 0, 0, 0, 'lfsworld_v1', $4, 'wheel', 0, '0.5A', 'valid'
            FROM chart WHERE era_id = $2",
        )
        .bind(player)
        .bind(era.id)
        .bind(60000 + index * 1000)
        .bind(format!("2006-01-01:{index}"))
        .execute(&pool)
        .await?;
    }
    era.rebuild_personal_bests(&database).await?;
    era.rebuild_badges(&database).await?;
    let awards = Model::list_for_players(&database, era.id, players.iter().copied()).await?;
    for (index, player) in players.iter().enumerate() {
        let badges = &awards[player];
        assert!(badges.iter().any(|b| b.badge_id == "gabor"));
        assert!(badges.iter().any(|b| b.badge_id == "newbie"));
        if index < 3 {
            let expected_variant = ["gold", "silver", "bronze"][index];
            assert!(
                badges
                    .iter()
                    .any(|b| b.badge_id == "mhr" && b.variant == expected_variant)
            );
            assert!(
                badges
                    .iter()
                    .any(|b| b.badge_id == "world-record-podium" && b.variant == expected_variant)
            );
        } else {
            assert!(
                !badges
                    .iter()
                    .any(|b| b.badge_id == "mhr" || b.badge_id == "world-record-podium")
            );
        }
        assert_eq!(
            badges.iter().any(|b| b.badge_id == "world-record-leader"),
            index == 0
        );
    }
    // Publication changes recalculate the projection, including podium promotions.
    grant_manual(
        &database,
        players[0],
        "mhr",
        "standard",
        Some("One-off grant"),
    )
    .await?;
    // Activity includes global and matching-era badges on every upload, even
    // when a driver appears more than once; awards from other eras stay out.
    let other_era: i64 = sqlx::query_scalar("INSERT INTO era (slug, title, installation_id, version_requirement, open) VALUES ('other-badge-era', 'Other era', '0.7', '>=0.7A', false) RETURNING id")
        .fetch_one(&pool).await?;
    let transaction = database.begin().await?;
    Model::replace_era(
        &transaction,
        other_era,
        HashMap::from([(
            players[0],
            vec![CalculatedAward::new("nutter", "gold", "other-era", None)],
        )]),
    )
    .await?;
    transaction.commit().await?;
    sqlx::query(
        "INSERT INTO hotlap (player_id, era_id, chart_id, track, vehicle, raw_vehicle_name,
        lap_time_ms, split_1_ms, split_2_ms, split_3_ms, split_4_ms, source, fingerprint,
        steering, player_flags, game_version, state)
        SELECT player_id, era_id, chart_id, track, vehicle, raw_vehicle_name,
        lap_time_ms + 1000, 0, 0, 0, 0, source, '2006-01-02:1',
        steering, player_flags, game_version, state
        FROM hotlap WHERE player_id = $1 LIMIT 1",
    )
    .bind(players[0])
    .execute(&pool)
    .await?;
    let page = crate::models::Hotlap::list(
        &database,
        crate::models::hotlap::HotlapListFilters::default(),
        crate::models::hotlap::HotlapListPage {
            column: crate::models::hotlap::HotlapListColumn::Submitted,
            order: lfsplanet_core::ordering::Ordering::Desc,
            offset: 0,
            limit: 100,
        },
    )
    .await?;
    let entries = page
        .entries
        .iter()
        .filter(|entry| entry.player.id == players[0])
        .collect::<Vec<_>>();
    assert_eq!(entries.len(), 2);
    for entry in &entries {
        let badges = entry.badges.as_ref().expect("activity badges loaded");
        assert!(
            badges
                .iter()
                .any(|badge| badge.description == "One-off grant")
        );
        assert!(badges.iter().any(|badge| badge.badge_id == "newbie"));
        assert!(!badges.iter().any(|badge| badge.badge_id == "nutter"));
    }
    assert_eq!(entries[0].badges, entries[1].badges);
    sqlx::query("DELETE FROM hotlap WHERE player_id = $1")
        .bind(players[0])
        .execute(&pool)
        .await?;
    era.rebuild_personal_bests(&database).await?;
    era.rebuild_badges(&database).await?;
    let updated = Model::list_for_players(&database, era.id, players.iter().copied()).await?;
    assert_eq!(updated[&players[0]].len(), 1);
    assert_eq!(updated[&players[0]][0].description, "One-off grant");
    assert!(
        updated[&players[1]]
            .iter()
            .any(|b| b.badge_id == "world-record-podium" && b.variant == "gold")
    );
    assert!(
        updated[&players[3]]
            .iter()
            .any(|b| b.badge_id == "mhr" && b.variant == "bronze")
    );
    Ok(())
}
