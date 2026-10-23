//! Narrow, complete reads of published chart personal bests for Pubstat.
use super::SteeringInput;
use crate::{track_id::TrackId, vehicle_id::VehicleId};
use insim_core::{track::Track, vehicle::Vehicle};
use sea_orm::{ActiveEnum, ConnectionTrait, DbBackend, DbErr, FromQueryResult, Statement};

#[derive(Debug, FromQueryResult)]
pub(crate) struct Row {
    pub id: i64,
    pub track: TrackId,
    pub car: VehicleId,
    pub split1: i64,
    pub split2: i64,
    pub split3: i64,
    pub laptime: i64,
    pub flags_hlaps: i32,
    pub racername: String,
    pub created_at: time::OffsetDateTime,
}

#[derive(Default)]
pub(crate) struct Filters<'a> {
    pub racer: Option<&'a str>,
    pub track: Option<Track>,
    pub car: Option<Vehicle>,
    pub steering: Option<SteeringInput>,
    pub country: Option<&'a str>,
    pub leaders_only: bool,
}

pub(crate) async fn list(
    database: &impl ConnectionTrait,
    era_id: i64,
    filters: Filters<'_>,
) -> Result<Vec<Row>, DbErr> {
    // PB membership supplies both current chart eligibility and best-per-player
    // semantics. No source filter: historical imports are public results too.
    let sql = r#"
SELECT h.id, chart.track_id AS track, chart.vehicle_id AS car,
       h.split_1_ms AS split1, h.split_2_ms AS split2, h.split_3_ms AS split3,
       h.lap_time_ms AS laptime, h.player_flags AS flags_hlaps,
       player.lfs_username AS racername, h.created_at
FROM hotlap_personal_best pb
JOIN chart ON chart.id = pb.chart_id
JOIN hotlap h ON h.id = pb.hotlap_id
    AND h.chart_id = chart.id AND h.era_id = chart.era_id
    AND h.player_id = pb.player_id
JOIN player ON player.id = pb.player_id
WHERE chart.era_id = $1 AND h.state = 'valid'
  AND ($2::TEXT IS NULL OR LOWER(player.lfs_username) = LOWER($2))
  AND ($3::TEXT IS NULL OR chart.track_id = $3)
  AND ($4::TEXT IS NULL OR chart.vehicle_id = $4)
  AND ($5::TEXT IS NULL OR h.steering = $5)
  AND ($6::TEXT IS NULL OR player.country_code = $6)
  AND (NOT $7::BOOLEAN OR pb.position = 1)
ORDER BY chart.track_id, chart.vehicle_id, pb.position
"#;
    Row::find_by_statement(Statement::from_sql_and_values(
        DbBackend::Postgres,
        sql,
        [
            era_id.into(),
            filters.racer.map(str::to_owned).into(),
            filters.track.map(TrackId).into(),
            filters.car.map(VehicleId).into(),
            filters.steering.map(|steering| steering.to_value()).into(),
            filters.country.map(str::to_owned).into(),
            filters.leaders_only.into(),
        ],
    ))
    .all(database)
    .await
}
