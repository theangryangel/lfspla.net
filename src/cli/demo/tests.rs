use super::{DemoArgs, persist};
use crate::models::hotlap::Entity as HotlapEntity;
use sea_orm::{EntityTrait, SqlxPostgresConnector};
#[sqlx::test]
#[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
async fn accumulates_demo_data_and_skips_duplicate_laps(pool: sqlx::PgPool) -> anyhow::Result<()> {
    let database = SqlxPostgresConnector::from_sqlx_postgres_pool(pool.clone());
    let store = object_store::memory::InMemory::new();
    let options = DemoArgs {
        yes: true,
        players: 100,
        coverage: 90,
        seed: Some(42),
    };
    sqlx::query("INSERT INTO vehicle (id, kind, name, normalized_name, license) VALUES ('ABCDEF', 'mod', 'Test mod', 'test mod', 's3')")
        .execute(&pool).await?;

    let inserted = persist::populate(&database, &options, &store).await?;
    let era = crate::models::Era::find_by_slug(&database, &"demo".parse().unwrap())
        .await?
        .unwrap();
    assert!(!era.open);
    let laps = HotlapEntity::find().all(&database).await?;
    assert_eq!(laps.len(), inserted);
    let invalid: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM hotlap WHERE era_id = $1 AND
         (source <> 'demo' OR spr_attachment_id IS NOT NULL OR hlvc_result_code IS NOT NULL OR
          split_1_ms >= split_2_ms OR split_3_ms <> lap_time_ms OR game_version <> '0.0A')",
    )
    .bind(era.id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(invalid, 0);

    let combinations: i64 =
        sqlx::query_scalar("SELECT count(*) FROM ranking_chart_membership WHERE era_id = $1")
            .bind(era.id)
            .fetch_one(&pool)
            .await?;
    let covered: i64 =
        sqlx::query_scalar("SELECT count(DISTINCT (track, vehicle)) FROM hotlap WHERE era_id = $1")
            .bind(era.id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(covered, (combinations * 90 + 99) / 100);
    let mod_charts: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM chart JOIN vehicle ON vehicle.id = chart.vehicle_id WHERE chart.era_id = $1 AND vehicle.kind = 'mod'",
    ).bind(era.id).fetch_one(&pool).await?;
    assert_eq!(mod_charts, 0);
    let personal_bests: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM hotlap_personal_best WHERE chart_id IN (SELECT id FROM chart WHERE era_id = $1) AND position > 0",
    )
    .bind(era.id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(usize::try_from(personal_bests)?, inserted);
    let badges: i64 = sqlx::query_scalar("SELECT count(*) FROM badge_award WHERE era_id = $1")
        .bind(era.id)
        .fetch_one(&pool)
        .await?;
    assert!(badges > 0);

    let events: Vec<(i64, String, time::OffsetDateTime, time::OffsetDateTime, i64)> = sqlx::query_as(
        "SELECT id,source,starts_at,ends_at,layout_attachment_id FROM event WHERE catalogue_id IN ('demo-bomb','demo-metronome','demo-shortcut','demo-land-speed-record') ORDER BY id"
    ).fetch_all(&pool).await?;
    assert_eq!(events.len(), 4);
    for (id, source, starts, ends, layout) in &events {
        let now = time::OffsetDateTime::now_utc();
        let monday = now.date().midnight().assume_utc()
            - time::Duration::days(i64::from(now.weekday().number_days_from_monday()));
        assert!(*starts >= monday && *starts < *ends && *ends < now);
        let kind = match source.as_str() {
            "bomb" => crate::models::event::game::EventKind::Bomb,
            "metronome" => crate::models::event::game::EventKind::Metronome,
            "land_speed_record" => crate::models::event::game::EventKind::LandSpeedRecord,
            _ => crate::models::event::game::EventKind::Shortcut,
        };
        let (standings, total) =
            crate::models::event::results::standings(&pool, *id, kind, 50, 0).await?;
        assert_eq!(total, 100);
        assert_eq!(standings.len(), 50);
        assert_eq!(standings[0].position, 1);
        if source == "land_speed_record" {
            assert!(
                standings
                    .windows(2)
                    .all(|rows| rows[0].speed_kph >= rows[1].speed_kph)
            );
            assert!(
                standings
                    .iter()
                    .all(|row| row.speed_kph.is_some_and(|speed| speed > 0.0))
            );
        }
        let attachment = crate::models::attachment::Model::required(&database, *layout).await?;
        let bytes = object_store::ObjectStoreExt::get(
            &store,
            &object_store::path::Path::from(attachment.object_key),
        )
        .await?
        .bytes()
        .await?;
        assert!(!insim_lyt::Lyt::read(bytes.as_ref())?.objects.is_empty());
    }
    let original_results: Vec<(i64, i64, i64)> = sqlx::query_as(
        "SELECT event_id,player_id,position FROM game_event_standing ORDER BY event_id,position",
    )
    .fetch_all(&pool)
    .await?;
    let attempts: i64 = sqlx::query_scalar("SELECT (SELECT count(*) FROM bomb_run)+(SELECT count(*) FROM metronome_result)+(SELECT count(*) FROM shortcut_time)+(SELECT count(*) FROM land_speed_record_run)").fetch_one(&pool).await?;
    assert_eq!(attempts, 1200);
    let second = persist::populate(&database, &options, &store).await?;
    assert_eq!(second, 0);
    assert_eq!(sqlx::query_scalar::<_,i64>("SELECT (SELECT count(*) FROM bomb_run)+(SELECT count(*) FROM metronome_result)+(SELECT count(*) FROM shortcut_time)+(SELECT count(*) FROM land_speed_record_run)").fetch_one(&pool).await?,attempts);
    let repeated_events: Vec<(i64, String, time::OffsetDateTime, time::OffsetDateTime, i64)> =
        sqlx::query_as(
            "SELECT id,source,starts_at,ends_at,layout_attachment_id FROM event ORDER BY id",
        )
        .fetch_all(&pool)
        .await?;
    assert_eq!(events, repeated_events);
    let repeated_results: Vec<(i64, i64, i64)> = sqlx::query_as(
        "SELECT event_id,player_id,position FROM game_event_standing ORDER BY event_id,position",
    )
    .fetch_all(&pool)
    .await?;
    assert_eq!(original_results, repeated_results);
    // Upgrade previously generated last-week fixtures without replacing scores.
    sqlx::query("UPDATE event SET starts_at=starts_at-interval '7 days',ends_at=ends_at-interval '7 days' WHERE catalogue_id IN ('demo-bomb','demo-metronome','demo-shortcut','demo-land-speed-record')").execute(&pool).await?;
    for query in [
        "UPDATE bomb_run SET recorded_at=recorded_at-interval '7 days'",
        "UPDATE metronome_result SET recorded_at=recorded_at-interval '7 days'",
        "UPDATE shortcut_time SET recorded_at=recorded_at-interval '7 days'",
        "UPDATE land_speed_record_run SET recorded_at=recorded_at-interval '7 days'",
    ] {
        sqlx::query(query).execute(&pool).await?;
    }
    assert_eq!(persist::populate(&database, &options, &store).await?, 0);
    let moved: Vec<(i64, String, time::OffsetDateTime, time::OffsetDateTime, i64)> =
        sqlx::query_as(
            "SELECT id,source,starts_at,ends_at,layout_attachment_id FROM event ORDER BY id",
        )
        .fetch_all(&pool)
        .await?;
    assert_eq!(events, moved);
    let outside: i64 = sqlx::query_scalar("SELECT count(*) FROM (SELECT event_id,recorded_at FROM bomb_run UNION ALL SELECT event_id,recorded_at FROM metronome_result UNION ALL SELECT event_id,recorded_at FROM shortcut_time UNION ALL SELECT event_id,recorded_at FROM land_speed_record_run) r JOIN event e ON e.id=r.event_id WHERE r.recorded_at<e.starts_at OR r.recorded_at>=e.ends_at").fetch_one(&pool).await?;
    assert_eq!(outside, 0);

    let repeated = HotlapEntity::find().all(&database).await?;
    assert_eq!(repeated.len(), laps.len());
    assert!(laps.iter().all(|lap| repeated.contains(lap)));
    let total_players: i64 =
        sqlx::query_scalar("SELECT count(*) FROM player WHERE deny_auth AND deny_uploads")
            .fetch_one(&pool)
            .await?;
    assert_eq!(total_players, 100);
    let original_positions: Vec<(i64, i64)> =
        sqlx::query_as("SELECT hotlap_id, position FROM hotlap_personal_best WHERE chart_id IN (SELECT id FROM chart WHERE era_id = $1)")
            .bind(era.id)
            .fetch_all(&pool)
            .await?;

    let added = persist::populate(
        &database,
        &DemoArgs {
            seed: Some(123),
            ..options
        },
        &store,
    )
    .await?;
    assert!(added > 0);
    let accumulated = HotlapEntity::find().all(&database).await?;
    assert_eq!(accumulated.len(), inserted + added);
    for lap in &laps {
        assert!(accumulated.contains(lap), "existing lap must be preserved");
    }
    let accumulated_players: i64 = sqlx::query_scalar("SELECT count(*) FROM player")
        .fetch_one(&pool)
        .await?;
    assert!(accumulated_players > total_players);
    let ranked: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM hotlap_personal_best WHERE chart_id IN (SELECT id FROM chart WHERE era_id = $1) AND position > 0",
    )
    .bind(era.id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(usize::try_from(ranked)?, accumulated.len());
    let updated_positions: std::collections::HashMap<i64, i64> = sqlx::query_as::<_, (i64, i64)>(
        "SELECT hotlap_id, position FROM hotlap_personal_best WHERE chart_id IN (SELECT id FROM chart WHERE era_id = $1)",
    )
    .bind(era.id)
    .fetch_all(&pool)
    .await?
    .into_iter()
    .collect();
    assert!(
        original_positions
            .iter()
            .any(|(id, position)| updated_positions[id] > *position),
        "new faster laps must push existing laps down the charts"
    );
    let out_of_order: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM (
            SELECT hotlap.lap_time_ms,
                lag(hotlap.lap_time_ms) OVER (
                    PARTITION BY pb.chart_id ORDER BY pb.position
                ) AS previous_time
            FROM hotlap_personal_best pb JOIN hotlap ON hotlap.id = pb.hotlap_id
            WHERE hotlap.era_id = $1
         ) ordered WHERE lap_time_ms < previous_time",
    )
    .bind(era.id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(out_of_order, 0, "chart positions must follow lap times");
    // Exercise the chart-based joins behind ranking and player API views.
    let ranking = crate::models::ranking::RankingWithCharts::find(&database, era.id, "all")
        .await?
        .unwrap();
    assert_eq!(ranking.charts.len() as i64, combinations);
    let nations = ranking.definition.nations(&database).await?;
    assert!(!nations.is_empty());
    for nation in nations {
        let contributions = ranking
            .definition
            .list_nation_contributions(&database, nation.country_code.as_str())
            .await?;
        assert_eq!(
            contributions.iter().map(|row| row.points).sum::<i64>(),
            nation.points
        );
    }
    let player = crate::models::player::Entity::find_by_id(laps[0].player_id)
        .one(&database)
        .await?
        .unwrap();
    let results = player
        .list_chart_results(&database, Some(era.id), None, None)
        .await?;
    assert!(!results.is_empty());
    assert_eq!(
        ranking
            .definition
            .list_personal_chart_bests(&database, &player)
            .await?
            .len(),
        results.len()
    );
    assert_eq!(
        player.list_era_stats(&database).await?[0].personal_bests as usize,
        results.len()
    );
    assert!(!era.list_podium_counts(&database).await?.is_empty());
    let activity = crate::models::Hotlap::list(
        &database,
        crate::models::hotlap::HotlapListFilters::default(),
        crate::models::hotlap::HotlapListPage {
            column: crate::models::hotlap::HotlapListColumn::Submitted,
            order: crate::ordering::Ordering::Desc,
            offset: 0,
            limit: 10,
        },
    )
    .await?;
    assert_eq!(activity.total as usize, accumulated.len());
    assert!(
        activity
            .entries
            .iter()
            .all(|entry| entry.position.is_some() && entry.contributes_to.len() == 1)
    );
    Ok(())
}

#[sqlx::test]
#[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
async fn existing_eras_are_not_populated_or_modified(pool: sqlx::PgPool) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO era (slug, title, version_requirement, installation_id, open)
        VALUES ('2026-09-30', 'Real era', '>=0.7A', '0.7', TRUE)",
    )
    .execute(&pool)
    .await?;
    let league: i64 =
        sqlx::query_scalar("INSERT INTO league (name) VALUES ('Actual competition') RETURNING id")
            .fetch_one(&pool)
            .await?;
    let layout: i64 = sqlx::query_scalar("INSERT INTO attachment(object_key,media_type,extension) VALUES ('real.lyt','application/octet-stream','lyt') RETURNING id").fetch_one(&pool).await?;
    let now = time::OffsetDateTime::now_utc();
    let monday = now.date().midnight().assume_utc()
        - time::Duration::days(i64::from(now.weekday().number_days_from_monday()));
    let real_end = monday + (now - monday) / 4_i32;
    let real_event: i64 = sqlx::query_scalar("INSERT INTO event(league_id,catalogue_id,title,starts_at,ends_at,source,game_config,layout_attachment_id) VALUES ($1,'actual-shortcut','Actual event',$3,$4,'shortcut','{\"source\":{\"type\":\"shortcut\"},\"track\":\"BL1\",\"vehicles\":[]}',$2) RETURNING id").bind(league).bind(layout).bind(monday).bind(real_end).fetch_one(&pool).await?;
    let real_before: (time::OffsetDateTime, time::OffsetDateTime, String) =
        sqlx::query_as("SELECT starts_at,ends_at,title FROM event WHERE id=$1")
            .bind(real_event)
            .fetch_one(&pool)
            .await?;
    let database = SqlxPostgresConnector::from_sqlx_postgres_pool(pool.clone());
    let store = object_store::memory::InMemory::new();
    let before = crate::models::Era::find_by_slug(&database, &"2026-09-30".parse().unwrap())
        .await?
        .unwrap();
    persist::populate(
        &database,
        &DemoArgs {
            yes: true,
            players: 1,
            coverage: 1,
            seed: Some(7),
        },
        &store,
    )
    .await?;
    let after = crate::models::Era::find_by_slug(&database, &"2026-09-30".parse().unwrap())
        .await?
        .unwrap();
    assert_eq!(before, after);
    let real_laps: i64 = sqlx::query_scalar("SELECT count(*) FROM hotlap WHERE era_id = $1")
        .bind(before.id)
        .fetch_one(&pool)
        .await?;
    assert_eq!(real_laps, 0);
    let real_after: (time::OffsetDateTime, time::OffsetDateTime, String) =
        sqlx::query_as("SELECT starts_at,ends_at,title FROM event WHERE id=$1")
            .bind(real_event)
            .fetch_one(&pool)
            .await?;
    assert_eq!(real_before, real_after);
    assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM event WHERE catalogue_id IN ('demo-bomb','demo-metronome','demo-shortcut','demo-land-speed-record') AND starts_at >= $1").bind(real_before.1).fetch_one(&pool).await?,4);

    Ok(())
}
