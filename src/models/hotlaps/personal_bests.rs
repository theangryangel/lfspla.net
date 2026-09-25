//! Maintenance for the current-personal-best projection.

use sea_orm::{ConnectionTrait, DbBackend, DbErr, Statement, Value};

use crate::models::eras;

/// Makes a newly valid hotlap its chart's personal best when it beats the
/// current row. The full comparator is deliberately shared with rebuilds.
pub(crate) async fn consider<C>(database: &C, hotlap_id: i64) -> Result<bool, DbErr>
where
    C: ConnectionTrait,
{
    // The caller holds the catalogue admission lock and uses a transaction.
    // Lock before the upsert, so the following rerank sees prior chart writes.
    let key = database
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT era_id, track, vehicle FROM hotlap WHERE id = $1 AND state = 'valid'",
            [hotlap_id.into()],
        ))
        .await?;
    let Some(key) = key else {
        return Ok(false);
    };
    let era_id: i64 = key.try_get("", "era_id")?;
    let track: String = key.try_get("", "track")?;
    let vehicle: String = key.try_get("", "vehicle")?;
    lock_chart(database, era_id, &track, &vehicle).await?;
    let changed = database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r"
INSERT INTO hotlap_personal_best (era_id, player_id, track, vehicle, hotlap_id)
SELECT era_id, player_id, track, vehicle, id
FROM hotlap
WHERE id = $1
  AND state = 'valid'
  AND EXISTS (
      SELECT 1 FROM ranking_chart
      WHERE ranking_chart.era_id = hotlap.era_id
        AND ranking_chart.track_id = hotlap.track
        AND ranking_chart.vehicle_id = hotlap.vehicle
  )
ON CONFLICT (era_id, player_id, track, vehicle) DO UPDATE
SET hotlap_id = EXCLUDED.hotlap_id
WHERE (
    SELECT ROW(lap_time_ms, created_at, id)
    FROM hotlap
    WHERE id = EXCLUDED.hotlap_id
) < (
    SELECT ROW(lap_time_ms, created_at, id)
    FROM hotlap
    WHERE id = hotlap_personal_best.hotlap_id
)
",
            [Value::from(hotlap_id)],
        ))
        .await?
        .rows_affected();
    if changed > 0 {
        eras::rerank_chart(database, era_id, Some(&track), Some(&vehicle)).await?;
    }
    // Even a slower published lap changes the "exactly one hotlap" badge.
    eras::request_badge_refresh(database, era_id).await?;
    let position: Option<i64> = database
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT position FROM hotlap_personal_best WHERE hotlap_id = $1",
            [Value::from(hotlap_id)],
        ))
        .await?
        .map(|row| row.try_get("", "position"))
        .transpose()?;
    Ok(position == Some(1))
}

/// Rebuilds a player's one chart after its present best was removed.
/// The caller holds the chart lock from before removal through commit.
pub(crate) async fn refresh_key<C>(
    database: &C,
    era_id: i64,
    player_id: i64,
    track: &str,
    vehicle: &str,
) -> Result<(), DbErr>
where
    C: ConnectionTrait,
{
    let values = [
        Value::from(era_id),
        Value::from(player_id),
        Value::from(track.to_owned()),
        Value::from(vehicle.to_owned()),
    ];
    database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r"
DELETE FROM hotlap_personal_best
WHERE era_id = $1 AND player_id = $2 AND track = $3 AND vehicle = $4
",
            values.clone(),
        ))
        .await?;
    database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r"
INSERT INTO hotlap_personal_best (era_id, player_id, track, vehicle, hotlap_id)
SELECT era_id, player_id, track, vehicle, id
FROM hotlap
WHERE era_id = $1 AND player_id = $2 AND track = $3 AND vehicle = $4
  AND state = 'valid'
  AND EXISTS (
      SELECT 1 FROM ranking_chart
      WHERE ranking_chart.era_id = hotlap.era_id
        AND ranking_chart.track_id = hotlap.track
        AND ranking_chart.vehicle_id = hotlap.vehicle
  )
ORDER BY lap_time_ms, created_at, id
LIMIT 1
",
            values,
        ))
        .await?;
    eras::rerank_chart(database, era_id, Some(track), Some(vehicle)).await?;
    eras::request_badge_refresh(database, era_id).await?;
    Ok(())
}

/// Serialize PB mutations for a chart. Acquire after catalogue admission and
/// before deleting or replacing a PB; retain through commit and reranking.
pub(crate) async fn lock_chart<C: ConnectionTrait>(
    database: &C,
    era_id: i64,
    track: &str,
    vehicle: &str,
) -> Result<(), DbErr> {
    database
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT pg_advisory_xact_lock(hashtextextended($1, 0))",
            [format!("hotlap_chart:{era_id}:{track}:{vehicle}").into()],
        ))
        .await?;
    Ok(())
}
