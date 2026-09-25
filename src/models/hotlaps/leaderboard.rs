//! Best-hotlap leaderboard models and queries.
use sea_orm::{AccessMode, ConnectionTrait, IsolationLevel, TransactionTrait};

use celes::Country;
use insim_core::{track::Track, vehicle::Vehicle};
use sea_orm::{ActiveEnum, DatabaseConnection, DbBackend, FromQueryResult, LoaderTrait, Statement};

use crate::models::Ordering;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::{HotlapModel, SteeringInput};
use crate::models::{
    players::{PlayerEntity, PlayerModel},
    rankings::benchmark_sql,
};

/// One player's fastest valid hotlap for a requested chart and era.
#[derive(Debug, Clone, PartialEq)]
pub struct BestHotlap {
    /// Global chart position, unchanged by filters or pagination.
    pub position: i64,
    pub hotlap: HotlapModel,
    pub player: PlayerModel,
    pub distance_to_benchmark_ms: i64,
    pub distance_to_world_record_ms: i64,
}

/// Filters applied to the chart's current personal bests.
pub struct BestHotlapFilters {
    pub era_id: i64,
    pub track: Track,
    pub vehicle: Vehicle,
    pub country: Option<Country>,
    pub controller: Option<SteeringInput>,
}

/// Supported chart sort keys, independent of direction.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum BestHotlapColumn {
    #[default]
    Rank,
    Driver,
    Set,
}

impl BestHotlapColumn {
    fn sql(self) -> &'static str {
        match self {
            Self::Rank => "pb.position",
            Self::Driver => "LOWER(player.display_name) COLLATE \"C\"",
            Self::Set => "hotlap.created_at",
        }
    }
}

/// Offset pagination requested for a best-hotlap query.
pub struct BestHotlapPage {
    pub offset: u64,
    pub limit: u64,
    pub column: BestHotlapColumn,
    pub order: Ordering,
}

/// A hotlap, chart position, and time gaps. Player details are loaded
/// in one query for the whole page.
#[derive(Debug, FromQueryResult)]
struct BestHotlapRow {
    position: i64,
    #[sea_orm(nested)]
    hotlap: HotlapModel,
    distance_to_benchmark_ms: i64,
    distance_to_world_record_ms: i64,
}

const CHART_FROM: &str = r"
FROM hotlap_personal_best pb
JOIN hotlap ON hotlap.id = pb.hotlap_id
JOIN player ON player.id = pb.player_id
";
const CHART_FILTER: &str = r"
WHERE pb.track = $1
  AND pb.vehicle = $2
  AND ($3::TEXT IS NULL OR hotlap.steering = $3)
  AND ($4::TEXT IS NULL OR player.country_code = $4)
  AND pb.era_id = $5
";

/// Lists current personal bests for one chart and era.
pub async fn list_best(
    database: &DatabaseConnection,
    filters: BestHotlapFilters,
    page: BestHotlapPage,
) -> Result<(Vec<BestHotlap>, u64), sea_orm::DbErr> {
    let transaction = database
        .begin_with_config(
            Some(IsolationLevel::RepeatableRead),
            Some(AccessMode::ReadOnly),
        )
        .await?;
    let result = list_best_in_snapshot(&transaction, filters, page).await?;
    transaction.commit().await?;
    Ok(result)
}

async fn list_best_in_snapshot(
    database: &impl ConnectionTrait,
    filters: BestHotlapFilters,
    page: BestHotlapPage,
) -> Result<(Vec<BestHotlap>, u64), sea_orm::DbErr> {
    let sql = format!(
        r"
SELECT hotlap.*, pb.position,
    hotlap.lap_time_ms - {benchmark} AS distance_to_benchmark_ms,
    hotlap.lap_time_ms - record.lap_time_ms AS distance_to_world_record_ms
{CHART_FROM}
CROSS JOIN (
    SELECT hotlap.lap_time_ms
    FROM hotlap_personal_best pb
    JOIN hotlap ON hotlap.id = pb.hotlap_id
    WHERE pb.track = $1 AND pb.vehicle = $2 AND pb.era_id = $5
      AND pb.position = 1
) record
{CHART_FILTER}
ORDER BY {column} {order}, pb.position ASC
LIMIT $6 OFFSET $7
",
        benchmark = benchmark_sql("record.lap_time_ms"),
        column = page.column.sql(),
        order = page.order.sql(),
    );

    // Shared filter parameters for both statements.
    let mut values: Vec<sea_orm::Value> = vec![
        filters.track.to_string().into(),
        filters.vehicle.to_string().into(),
        filters.controller.map(SteeringInput::into_value).into(),
        filters
            .country
            .map(|country| country.alpha2.to_owned())
            .into(),
        filters.era_id.to_owned().into(),
    ];

    let total = database
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            format!("SELECT COUNT(*)::BIGINT AS total {CHART_FROM} {CHART_FILTER}"),
            values.clone(),
        ))
        .await?
        .ok_or_else(|| sea_orm::DbErr::RecordNotFound("chart count missing".into()))?
        .try_get::<i64>("", "total")?;
    values.push(
        i64::try_from(page.limit)
            .map_err(|e| sea_orm::DbErr::Type(e.to_string()))?
            .into(),
    );
    values.push(
        i64::try_from(page.offset)
            .map_err(|e| sea_orm::DbErr::Type(e.to_string()))?
            .into(),
    );

    let rows = BestHotlapRow::find_by_statement(Statement::from_sql_and_values(
        DbBackend::Postgres,
        sql,
        values,
    ))
    .all(database)
    .await?;

    let (hotlaps, distances): (Vec<_>, Vec<_>) = rows
        .into_iter()
        .map(|row| {
            (
                row.hotlap,
                (
                    row.position,
                    row.distance_to_benchmark_ms,
                    row.distance_to_world_record_ms,
                ),
            )
        })
        .unzip();
    // One further query for the whole page's owners, through the relation the
    // hotlap entity declares.
    let players = hotlaps.load_one(PlayerEntity, database).await?;

    let entries = hotlaps
        .into_iter()
        .zip(distances)
        .zip(players)
        .map(|((hotlap, (position, benchmark, record)), player)| {
            // `Restrict` on the relation means a lap cannot outlive its owner,
            // so an absent player here is a broken row.
            let player = player.ok_or_else(|| {
                sea_orm::DbErr::Type(format!("hotlap {} has no player", hotlap.id))
            })?;
            Ok(BestHotlap {
                position,
                hotlap,
                player,
                distance_to_benchmark_ms: benchmark,
                distance_to_world_record_ms: record,
            })
        })
        .collect::<Result<Vec<_>, sea_orm::DbErr>>()?;
    Ok((
        entries,
        u64::try_from(total).map_err(|e| sea_orm::DbErr::Type(e.to_string()))?,
    ))
}
