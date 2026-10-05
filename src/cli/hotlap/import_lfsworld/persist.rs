//! Transactional persistence for preflighted historical hotlaps.

use super::ImportData;
use anyhow::Context;
use sea_orm::{
    ActiveEnum, DatabaseConnection,
    sqlx::{self, Postgres, QueryBuilder},
};
use std::collections::{HashMap, HashSet};
const SOURCE: &str = "lfsworld_v1";

#[allow(clippy::too_many_lines)]
pub(super) async fn persist(
    database: &DatabaseConnection,
    data: &ImportData,
) -> anyhow::Result<u64> {
    let pool = database.get_postgres_connection_pool();
    let mut transaction = pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(crate::db::CATALOGUE_LOCK)
        .execute(&mut *transaction)
        .await?;
    let charts: HashMap<(i64, String, String), i64> =
        sqlx::query_as::<_, (i64, i64, String, String)>(
            "SELECT id, era_id, track_id, vehicle_id FROM chart",
        )
        .fetch_all(&mut *transaction)
        .await?
        .into_iter()
        .map(|(id, era, track, vehicle)| ((era, track, vehicle), id))
        .collect();
    for hotlap in &data.hotlaps {
        anyhow::ensure!(
            charts.contains_key(&(hotlap.era_id, hotlap.track.clone(), hotlap.vehicle.clone())),
            "combination {}/{} is no longer eligible for era {}",
            hotlap.track,
            hotlap.vehicle,
            hotlap.era_id
        );
    }
    let mut player_ids = HashMap::with_capacity(data.players.len());

    for (username, player) in &data.players {
        let result = sqlx::query_as::<_, (i64,)>(
            r"
            INSERT INTO player (
                lfs_username, display_name, country_code, created_at,
                last_authenticated_at, lfsworld_id, flag_code
            )
            VALUES ($1, $1, $2, $3, NULL, $4, $5)
            ON CONFLICT (LOWER(lfs_username)) DO UPDATE
            SET lfsworld_id = COALESCE(player.lfsworld_id, EXCLUDED.lfsworld_id),
                country_code = CASE
                    -- A successful authentication is a newer, authoritative
                    -- profile observation than any checked-in export, but a
                    -- missing nation is not an observation and may be filled.
                    WHEN player.last_authenticated_at IS NOT NULL
                        THEN COALESCE(player.country_code, EXCLUDED.country_code)
                    -- Imports run from oldest to newest, so the latest
                    -- supplied snapshot owns an unauthenticated profile.
                    ELSE EXCLUDED.country_code
                END,
                flag_code = CASE
                    -- Keep signed-in players' choices, including null.
                    WHEN player.last_authenticated_at IS NOT NULL
                        THEN player.flag_code
                    ELSE EXCLUDED.flag_code
                END
            WHERE player.lfsworld_id IS NULL
               OR player.lfsworld_id = EXCLUDED.lfsworld_id
            RETURNING id
            ",
        )
        .bind(username)
        .bind(&player.country_code)
        .bind(player.created_at)
        .bind(player.lfsworld_id)
        .bind(&player.flag_code)
        .fetch_optional(&mut *transaction)
        .await?
        .with_context(|| {
            format!("player {username:?} already has a different LFSWorld identity")
        })?;
        player_ids.insert(username.as_str(), result.0);
    }

    // Imported laps may be reassigned to another era/chart. Remove their old
    // projections before updating the FK; the affected eras are rebuilt below.
    let fingerprints = data
        .hotlaps
        .iter()
        .map(|lap| lap.fingerprint.clone())
        .collect::<Vec<_>>();
    let old_eras: Vec<i64> = sqlx::query_scalar(
        "SELECT DISTINCT era_id FROM hotlap WHERE source = $1 AND fingerprint = ANY($2)",
    )
    .bind(SOURCE)
    .bind(&fingerprints)
    .fetch_all(&mut *transaction)
    .await?;
    sqlx::query(
        "DELETE FROM hotlap_personal_best pb USING hotlap h
         WHERE pb.hotlap_id = h.id AND h.source = $1 AND h.fingerprint = ANY($2)",
    )
    .bind(SOURCE)
    .bind(&fingerprints)
    .execute(&mut *transaction)
    .await?;
    let mut inserted = 0;
    for chunk in data.hotlaps.chunks(1_000) {
        let mut query = QueryBuilder::<Postgres>::new(
            r"
            INSERT INTO hotlap (
                player_id, era_id, chart_id, track, vehicle, raw_vehicle_name,
                lap_time_ms, split_1_ms, split_2_ms, split_3_ms, split_4_ms,
                original_filename, spr_object_key,
                source, fingerprint, steering, abs_enabled, player_flags,
                created_at, game_version, state
            )
            ",
        );
        query.push_values(chunk, |mut values, hotlap| {
            values
                .push_bind(player_ids[hotlap.lfs_username.as_str()])
                .push_bind(hotlap.era_id)
                .push_bind(charts[&(hotlap.era_id, hotlap.track.clone(), hotlap.vehicle.clone())])
                .push_bind(&hotlap.track)
                .push_bind(&hotlap.vehicle)
                .push_bind(&hotlap.vehicle)
                .push_bind(hotlap.lap_time_ms)
                .push_bind(hotlap.split_times_ms[0])
                .push_bind(hotlap.split_times_ms[1])
                .push_bind(hotlap.split_times_ms[2])
                .push_bind(hotlap.split_times_ms[3])
                .push_bind(&hotlap.original_filename)
                .push("NULL")
                .push_bind(SOURCE)
                .push_bind(&hotlap.fingerprint)
                .push_bind(hotlap.steering.into_value())
                .push_bind(hotlap.abs_enabled)
                .push_bind(i32::from(hotlap.player_flags))
                .push_bind(hotlap.created_at)
                .push_bind(&hotlap.game_version)
                .push("'valid'");
        });
        query.push(
            r"
            ON CONFLICT (source, fingerprint) DO UPDATE
            SET era_id = EXCLUDED.era_id,
                chart_id = EXCLUDED.chart_id,
                steering = EXCLUDED.steering,
                abs_enabled = EXCLUDED.abs_enabled,
                player_flags = EXCLUDED.player_flags
            WHERE hotlap.era_id IS DISTINCT FROM EXCLUDED.era_id
               OR hotlap.steering IS DISTINCT FROM EXCLUDED.steering
               OR hotlap.abs_enabled IS DISTINCT FROM EXCLUDED.abs_enabled
               OR hotlap.player_flags IS DISTINCT FROM EXCLUDED.player_flags
            ",
        );
        inserted += query
            .build()
            .execute(&mut *transaction)
            .await?
            .rows_affected();
    }

    // Imports publish rows directly rather than passing through HLVC, so they
    // must maintain the same current-personal-best projection explicitly.
    let era_ids = data
        .hotlaps
        .iter()
        .map(|hotlap| hotlap.era_id)
        .chain(old_eras)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    sqlx::query("DELETE FROM hotlap_personal_best WHERE chart_id IN (SELECT id FROM chart WHERE era_id = ANY($1))")
        .bind(&era_ids)
        .execute(&mut *transaction)
        .await?;
    sqlx::query(
        r"
INSERT INTO hotlap_personal_best (chart_id, player_id, hotlap_id)
SELECT DISTINCT ON (hotlap.chart_id, hotlap.player_id)
    hotlap.chart_id, hotlap.player_id, hotlap.id
FROM hotlap
WHERE hotlap.era_id = ANY($1)
  AND hotlap.state = 'valid'
ORDER BY
    hotlap.chart_id, hotlap.player_id,
    hotlap.lap_time_ms, hotlap.created_at, hotlap.id
",
    )
    .bind(&era_ids)
    .execute(&mut *transaction)
    .await?;

    for era_id in &era_ids {
        sqlx::query(crate::models::chart::RERANK_SQL)
            .bind(era_id)
            .bind(None::<i64>)
            .execute(&mut *transaction)
            .await?;
        sqlx::query("INSERT INTO era_badge_refresh (era_id) VALUES ($1) ON CONFLICT DO NOTHING")
            .bind(era_id)
            .execute(&mut *transaction)
            .await?;
    }
    transaction.commit().await?;
    Ok(inserted)
}

#[cfg(test)]
mod tests {
    use super::super::{ImportedHotlap, ImportedPlayer};
    use super::*;
    use crate::{
        milliseconds::Milliseconds,
        models::{Era, era::definition},
    };
    use sea_orm::SqlxPostgresConnector;

    #[sqlx::test]
    #[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
    async fn imports_resolve_shared_chart_and_rebuild_both_eras_when_reclassified(
        pool: sqlx::PgPool,
    ) -> anyhow::Result<()> {
        let database = SqlxPostgresConnector::from_sqlx_postgres_pool(pool.clone());
        crate::models::Track::sync(&database).await?;
        crate::models::Vehicle::sync_builtin(&database).await?;
        let desired = definition::test_definitions();
        crate::services::apply_eras::apply(&database, &desired, |_| Ok(())).await?;
        let first = Era::find_by_slug(&database, &desired[0].id).await?.unwrap();
        let second = Era::find_by_slug(&database, &desired[1].id).await?.unwrap();
        let pair = desired[0].rankings[0].combinations.pairs().next().unwrap();
        let date = time::OffsetDateTime::now_utc();
        let mut data = ImportData {
            players: [(
                "import-chart-test".to_owned(),
                ImportedPlayer {
                    lfsworld_id: 123,
                    country_code: Some("GB".into()),
                    flag_code: None,
                    created_at: date,
                },
            )]
            .into_iter()
            .collect(),
            hotlaps: vec![ImportedHotlap {
                fingerprint: "2006-01-01:1".into(),
                lfs_username: "import-chart-test".into(),
                era_slug: first.slug.clone(),
                era_id: first.id,
                track: pair.track.to_string(),
                vehicle: pair.vehicle.to_string(),
                lap_time_ms: Milliseconds::from_millis(60000),
                split_times_ms: [Milliseconds::ZERO; 4],
                original_filename: "import.spr".into(),
                steering: crate::models::hotlap::SteeringInput::Wheel,
                abs_enabled: None,
                player_flags: 0,
                created_at: date,
                game_version: "0.5P".into(),
            }],
        };
        assert_eq!(persist(&database, &data).await?, 1);
        assert_eq!(persist(&database, &data).await?, 0);
        let chart = crate::models::Chart::find_combination(
            &database,
            first.id,
            &pair.track.to_string(),
            &pair.vehicle.to_string(),
        )
        .await?
        .unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT chart_id FROM hotlap_personal_best")
                .fetch_one(&pool)
                .await?,
            chart.id
        );
        // Test definitions select the same pair in their first two eras.
        data.hotlaps[0].era_id = second.id;
        data.hotlaps[0].era_slug = second.slug;
        assert_eq!(persist(&database, &data).await?, 1);
        let (lap_chart, best_chart, era): (i64, i64, i64) = sqlx::query_as(
            "SELECT hotlap.chart_id, pb.chart_id, hotlap.era_id FROM hotlap
             JOIN hotlap_personal_best pb ON pb.hotlap_id = hotlap.id",
        )
        .fetch_one(&pool)
        .await?;
        assert_eq!(lap_chart, best_chart);
        assert_ne!(lap_chart, chart.id);
        assert_eq!(era, second.id);
        // Era repair must remap chart_id and the classification in one update,
        // removing and rebuilding the old PB before changing its FK.
        let repaired = crate::services::repair_eras::repair(&database).await?;
        assert_eq!(repaired.hotlaps_updated, 1);
        let (lap_chart, best_chart, era): (i64, i64, i64) = sqlx::query_as(
            "SELECT hotlap.chart_id, pb.chart_id, hotlap.era_id FROM hotlap
             JOIN hotlap_personal_best pb ON pb.hotlap_id = hotlap.id",
        )
        .fetch_one(&pool)
        .await?;
        assert_eq!(lap_chart, chart.id);
        assert_eq!(best_chart, chart.id);
        assert_eq!(era, first.id);
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM hotlap_personal_best")
                .fetch_one(&pool)
                .await?,
            1
        );
        Ok(())
    }
}
