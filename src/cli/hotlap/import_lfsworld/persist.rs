//! Transactional persistence for preflighted historical hotlaps.

use std::collections::{HashMap, HashSet};

use anyhow::Context;
use sea_orm::{
    ActiveEnum, DatabaseConnection,
    sqlx::{self, Postgres, QueryBuilder},
};

use super::ImportData;

const SOURCE: &str = "lfsworld_v1";

#[allow(clippy::too_many_lines)]
pub(super) async fn persist(
    database: &DatabaseConnection,
    data: &ImportData,
) -> anyhow::Result<u64> {
    let pool = database.get_postgres_connection_pool();
    let mut transaction = pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(crate::models::eras::CATALOGUE_LOCK)
        .execute(&mut *transaction)
        .await?;
    let eligible: HashSet<(i64, String, String)> =
        sqlx::query_as("SELECT DISTINCT era_id, track_id, vehicle_id FROM ranking_chart")
            .fetch_all(&mut *transaction)
            .await?
            .into_iter()
            .collect();
    for hotlap in &data.hotlaps {
        anyhow::ensure!(
            eligible.contains(&(hotlap.era_id, hotlap.track.clone(), hotlap.vehicle.clone())),
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
                last_authenticated_at, lfsworld_id
            )
            VALUES ($1, $1, $2, $3, NULL, $4)
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
        .fetch_optional(&mut *transaction)
        .await?
        .with_context(|| {
            format!("player {username:?} already has a different LFSWorld identity")
        })?;
        player_ids.insert(username.as_str(), result.0);
    }

    let mut inserted = 0;
    for chunk in data.hotlaps.chunks(1_000) {
        let mut query = QueryBuilder::<Postgres>::new(
            r"
            INSERT INTO hotlap (
                player_id, era_id, track, vehicle, raw_vehicle_name,
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
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    sqlx::query("DELETE FROM hotlap_personal_best WHERE era_id = ANY($1)")
        .bind(&era_ids)
        .execute(&mut *transaction)
        .await?;
    sqlx::query(
        r"
INSERT INTO hotlap_personal_best (era_id, player_id, track, vehicle, hotlap_id)
SELECT DISTINCT ON (hotlap.era_id, hotlap.player_id, hotlap.track, hotlap.vehicle)
    hotlap.era_id, hotlap.player_id, hotlap.track, hotlap.vehicle, hotlap.id
FROM hotlap
WHERE hotlap.era_id = ANY($1)
  AND hotlap.state = 'valid'
  AND EXISTS (
      SELECT 1 FROM ranking_chart
      WHERE ranking_chart.era_id = hotlap.era_id
        AND ranking_chart.track_id = hotlap.track
        AND ranking_chart.vehicle_id = hotlap.vehicle
  )
ORDER BY
    hotlap.era_id, hotlap.player_id, hotlap.track, hotlap.vehicle,
    hotlap.lap_time_ms, hotlap.created_at, hotlap.id
",
    )
    .bind(&era_ids)
    .execute(&mut *transaction)
    .await?;

    for era_id in &era_ids {
        sqlx::query(crate::models::eras::RERANK_SQL)
            .bind(era_id)
            .bind(None::<&str>)
            .bind(None::<&str>)
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
