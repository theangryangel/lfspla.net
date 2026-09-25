//! Era-wide maintenance for the current-personal-best projection.

use sea_orm::{ConnectionTrait, DbBackend, DbErr, Statement, Value};

use super::EraModel;

impl EraModel {
    /// Rebuilds this era's personal bests after its chart catalogue or
    /// classification changes. The caller holds the exclusive catalogue lock.
    pub(crate) async fn rebuild_personal_bests<C: ConnectionTrait>(
        &self,
        database: &C,
    ) -> Result<(), DbErr> {
        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "DELETE FROM hotlap_personal_best WHERE era_id = $1",
                [Value::from(self.id)],
            ))
            .await?;
        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                r"
INSERT INTO hotlap_personal_best (era_id, player_id, track, vehicle, hotlap_id)
SELECT DISTINCT ON (hotlap.era_id, hotlap.player_id, hotlap.track, hotlap.vehicle)
    hotlap.era_id, hotlap.player_id, hotlap.track, hotlap.vehicle, hotlap.id
FROM hotlap
WHERE hotlap.era_id = $1
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
                [Value::from(self.id)],
            ))
            .await?;
        self.rerank_personal_bests(database).await?;
        super::request_badge_refresh(database, self.id).await
    }

    /// Re-ranks every chart in this era. The caller holds the exclusive
    /// catalogue lock until commit.
    pub(crate) async fn rerank_personal_bests<C: ConnectionTrait>(
        &self,
        database: &C,
    ) -> Result<(), DbErr> {
        rerank(database, self.id, None, None).await
    }
}

/// Shared by normal PB maintenance, catalogue/re-era repairs and SQLx imports.
/// Call inside the transaction holding a chart lock or exclusive catalogue lock.
// Equal lap times compare upload date, then LFS username. Splits do not break ties.
pub(crate) const RERANK_SQL: &str = r#"
WITH positions AS (
    SELECT pb.hotlap_id,
        ROW_NUMBER() OVER (
            PARTITION BY pb.era_id, pb.track, pb.vehicle
            ORDER BY hotlap.lap_time_ms, hotlap.created_at, player.lfs_username COLLATE "C"
        ) AS position
    FROM hotlap_personal_best pb
    JOIN hotlap ON hotlap.id = pb.hotlap_id
    JOIN player ON player.id = pb.player_id
    WHERE pb.era_id = $1
      AND ($2::TEXT IS NULL OR pb.track = $2)
      AND ($3::TEXT IS NULL OR pb.vehicle = $3)
)
UPDATE hotlap_personal_best pb
SET position = positions.position
FROM positions
WHERE pb.hotlap_id = positions.hotlap_id
  AND pb.position IS DISTINCT FROM positions.position
"#;

/// Re-ranks one chart after its personal-best rows change.
pub(crate) async fn rerank_chart<C: ConnectionTrait>(
    database: &C,
    era_id: i64,
    track: Option<&str>,
    vehicle: Option<&str>,
) -> Result<(), DbErr> {
    rerank(database, era_id, track, vehicle).await
}

async fn rerank<C: ConnectionTrait>(
    database: &C,
    era_id: i64,
    track: Option<&str>,
    vehicle: Option<&str>,
) -> Result<(), DbErr> {
    database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            RERANK_SQL,
            [
                Value::from(era_id),
                Value::from(track),
                Value::from(vehicle),
            ],
        ))
        .await?;
    Ok(())
}
