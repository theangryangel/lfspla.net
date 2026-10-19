use super::*;
use crate::models::ranking::{RankingBadgeDefinition, RankingBadgeQualification};
#[test]
fn complete_badges_require_every_chart() {
    assert!(!qualifies_for_ranking_badge(
        RankingBadgeQualification::Complete,
        17,
        18,
        0
    ));
    assert!(qualifies_for_ranking_badge(
        RankingBadgeQualification::Complete,
        18,
        18,
        0
    ));
}

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

#[test]
fn ranked_badges_bound_the_requested_results() {
    let limited: RankingBadgeQualification =
        serde_json::from_str(r#"{"type":"ranked","limit":10}"#).unwrap();
    let unlimited: RankingBadgeQualification =
        serde_json::from_str(r#"{"type":"ranked"}"#).unwrap();

    assert_eq!(limited.result_limit(), 10);
    assert_eq!(unlimited.result_limit(), u64::MAX);
    assert_eq!(RankingBadgeQualification::Complete.result_limit(), u64::MAX);
}

#[sqlx::test]
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
    let profile = crate::models::player::Entity::find_by_id(player)
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

#[sqlx::test(migrations = false)]
#[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
async fn upgrade_preserves_existing_data_and_queues_the_new_badge_projection(
    pool: sqlx::PgPool,
) -> anyhow::Result<()> {
    let mut migrations = sqlx::migrate::Migrator::new(std::path::Path::new("migrations")).await?;
    migrations.migrations = std::borrow::Cow::Owned(
        migrations
            .migrations
            .iter()
            .filter(|m| m.version < 20261019150000)
            .cloned()
            .collect(),
    );
    migrations.run(&pool).await?;
    // Reproduce rows authored by the deployed version, including completion configuration.
    let era: i64 = sqlx::query_scalar("INSERT INTO era (slug, title, installation_id, version_requirement, open) VALUES ('badge-upgrade', 'Upgrade era', '0.7', '>=0.7A', false) RETURNING id")
        .fetch_one(&pool).await?;
    let player: i64 = sqlx::query_scalar("INSERT INTO player (lfs_username, display_name) VALUES ('badge-upgrade', 'Upgrade') RETURNING id")
        .fetch_one(&pool).await?;
    sqlx::query("INSERT INTO ranking (era_id, slug, position, title, description, badge, benchmark_percent, nation_max_points, nation_driver_limit) VALUES ($1, 'nutter', 0, 'Nutter Rank', 'Test', $2, 103, 10, 3)")
        .bind(era).bind(serde_json::json!({"label":"NUTTER", "qualification":{"type":"ranked", "limit":3}, "completion":{"label":"GABOR"}})).execute(&pool).await?;
    sqlx::query("INSERT INTO ranking (era_id, slug, position, title, description, badge, benchmark_percent, nation_max_points, nation_driver_limit) VALUES ($1, 'all', 1, 'Demo ranking', 'Custom charts', $2, 103, 10, 3)")
        .bind(era).bind(serde_json::json!({"label":"Demo", "qualification":{"type":"ranked", "limit":3}, "completion":{"label":"All done"}})).execute(&pool).await?;
    sqlx::query("INSERT INTO player_era_badges (era_id, player_id, badges) VALUES ($1, $2, '[{\"kind\":\"newbie\"}]')")
        .bind(era).bind(player).execute(&pool).await?;
    // Apply creation first, then the independently deployable cleanup migration.
    let mut schema = sqlx::migrate::Migrator::new(std::path::Path::new("migrations")).await?;
    schema.migrations = std::borrow::Cow::Owned(
        schema
            .migrations
            .iter()
            .filter(|m| m.version <= 20261019150000)
            .cloned()
            .collect(),
    );
    schema.run(&pool).await?;
    assert!(
        sqlx::query_scalar::<_, Option<String>>(
            "SELECT to_regclass('player_era_badges_legacy')::text"
        )
        .fetch_one(&pool)
        .await?
        .is_some()
    );
    sqlx::migrate::Migrator::new(std::path::Path::new("migrations"))
        .await?
        .run(&pool)
        .await?;
    assert!(
        sqlx::query_scalar::<_, Option<String>>(
            "SELECT to_regclass('player_era_badges_legacy')::text"
        )
        .fetch_one(&pool)
        .await?
        .is_none()
    );
    assert!(
        sqlx::query_scalar::<_, Option<String>>("SELECT to_regclass('player_era_badges')::text")
            .fetch_one(&pool)
            .await?
            .is_none()
    );
    for variant in ["standard", "gold", "silver", "bronze"] {
        assert!(
            sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS (SELECT 1 FROM badge_variant WHERE badge_id = 'all' AND id = $1)"
            )
            .bind(variant)
            .fetch_one(&pool)
            .await?
        );
    }
    assert!(sqlx::query_scalar::<_, bool>("SELECT EXISTS (SELECT 1 FROM badge_variant WHERE badge_id = 'all-completion' AND id = 'standard')")
        .fetch_one(&pool).await?);
    let database = sea_orm::SqlxPostgresConnector::from_sqlx_postgres_pool(pool.clone());
    // Existing custom references must not prevent unrelated catalogue seeding.
    crate::services::catalogue::seed(&database, &[], None, true, |_| Ok(())).await?;
    let value: serde_json::Value =
        sqlx::query_scalar("SELECT badge FROM ranking WHERE era_id = $1 AND slug = 'nutter'")
            .bind(era)
            .fetch_one(&pool)
            .await?;
    let definition: RankingBadgeDefinition = serde_json::from_value(value)?;
    assert_eq!(definition.id, "nutter");
    assert_eq!(definition.completion.unwrap().id, "gabor");
    assert_eq!(definition.qualification.result_limit(), 3);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT id FROM player WHERE id = $1")
            .bind(player)
            .fetch_one(&pool)
            .await?,
        player
    );
    assert!(
        sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS (SELECT 1 FROM era_badge_refresh WHERE era_id = $1)"
        )
        .bind(era)
        .fetch_one(&pool)
        .await?
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM badge_award")
            .fetch_one(&pool)
            .await?,
        0
    );
    Ok(())
}

#[sqlx::test]
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
            order: crate::ordering::Ordering::Desc,
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
