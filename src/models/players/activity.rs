//! Player activity and chart-achievement queries.

use std::collections::HashMap;

use sea_orm::{
    ConnectionTrait, DatabaseConnection, DbBackend, EntityTrait, FromQueryResult, Statement,
};

use crate::models::badges::{self, BadgeFilter, PlayerBadge, PlayerEraBadgesEntity};

use super::PlayerModel;

/// Activity and current chart achievements for one player in an era.
#[derive(Debug, Clone, PartialEq, Eq, FromQueryResult)]
pub(crate) struct PlayerEraStats {
    pub era_id: i64,
    pub hotlaps: i64,
    pub personal_bests: i64,
    pub first_hotlap_at: time::OffsetDateTime,
    pub latest_hotlap_at: time::OffsetDateTime,
}

/// One of a player's strongest current personal-best chart results.
#[derive(Debug, Clone, PartialEq, Eq, FromQueryResult)]
pub(crate) struct PlayerChartResult {
    pub hotlap_id: i64,
    pub downloadable: bool,
    pub era_id: i64,
    pub era_slug: String,
    pub track: String,
    pub vehicle: String,
    pub lap_time_ms: i64,
    pub distance_to_world_record_ms: i64,
    pub position: i64,
    pub entries: i64,
    pub created_at: time::OffsetDateTime,
    pub game_version: String,
}

impl PlayerModel {
    /// Returns this player's stored badges grouped by era.
    pub(crate) async fn list_badges_by_era(
        &self,
        database: &DatabaseConnection,
    ) -> Result<HashMap<i64, Vec<PlayerBadge>>, sea_orm::DbErr> {
        PlayerEraBadgesEntity::find()
            .owned_by(self.id)
            .all(database)
            .await?
            .into_iter()
            .map(|row| Ok((row.era_id, badges::decode(row)?)))
            .collect()
    }

    /// Summarises this player's activity by era.
    pub(crate) async fn list_era_stats(
        &self,
        database: &DatabaseConnection,
    ) -> Result<Vec<PlayerEraStats>, sea_orm::DbErr> {
        let sql = r"
WITH activity AS (
    SELECT
        era_id,
        COUNT(*)::BIGINT AS hotlaps,
        MIN(created_at) AS first_hotlap_at,
        MAX(created_at) AS latest_hotlap_at
    FROM hotlap
    WHERE player_id = $1
      AND state = 'valid' AND EXISTS (SELECT 1 FROM ranking_chart WHERE era_id = hotlap.era_id AND track_id = hotlap.track AND vehicle_id = hotlap.vehicle)
    GROUP BY era_id
),
personal_bests AS (
    SELECT era_id, COUNT(*)::BIGINT AS personal_bests
    FROM hotlap_personal_best
    WHERE player_id = $1
    GROUP BY era_id
)
SELECT
    activity.era_id,
    activity.hotlaps,
    personal_bests.personal_bests,
    activity.first_hotlap_at,
    activity.latest_hotlap_at
FROM activity
JOIN personal_bests USING (era_id)
ORDER BY activity.era_id DESC
";
        PlayerEraStats::find_by_statement(Statement::from_sql_and_values(
            DbBackend::Postgres,
            sql,
            [self.id.into()],
        ))
        .all(database)
        .await
    }

    /// Lists this player's current chart positions within optional chart filters.
    /// Stored PB positions are the authority, including chart tie breaks.
    pub(crate) async fn list_chart_results(
        &self,
        database: &impl ConnectionTrait,
        era_id: Option<i64>,
        track: Option<&str>,
        vehicle: Option<&str>,
    ) -> Result<Vec<PlayerChartResult>, sea_orm::DbErr> {
        let sql = r"
SELECT
    hotlap.id AS hotlap_id,
    (hotlap.spr_object_key IS NOT NULL) AS downloadable,
    pb.era_id,
    era.slug AS era_slug,
    pb.track,
    pb.vehicle,
    hotlap.lap_time_ms,
    hotlap.lap_time_ms - record.lap_time_ms AS distance_to_world_record_ms,
    pb.position,
    (SELECT COUNT(*) FROM hotlap_personal_best entries
     WHERE entries.era_id = pb.era_id AND entries.track = pb.track
       AND entries.vehicle = pb.vehicle) AS entries,
    hotlap.created_at,
    hotlap.game_version
FROM hotlap_personal_best pb
JOIN era ON era.id = pb.era_id
JOIN hotlap ON hotlap.id = pb.hotlap_id
JOIN hotlap_personal_best leader
  ON leader.era_id = pb.era_id AND leader.track = pb.track
 AND leader.vehicle = pb.vehicle AND leader.position = 1
JOIN hotlap record ON record.id = leader.hotlap_id
WHERE pb.player_id = $1
  AND ($2::BIGINT IS NULL OR pb.era_id = $2)
  AND ($3::TEXT IS NULL OR pb.track = $3)
  AND ($4::TEXT IS NULL OR pb.vehicle = $4)
ORDER BY pb.position, hotlap.created_at DESC, pb.era_id DESC, pb.track, pb.vehicle
";
        PlayerChartResult::find_by_statement(Statement::from_sql_and_values(
            DbBackend::Postgres,
            sql,
            [
                self.id.into(),
                era_id.into(),
                track.map(str::to_owned).into(),
                vehicle.map(str::to_owned).into(),
            ],
        ))
        .all(database)
        .await
    }
}
