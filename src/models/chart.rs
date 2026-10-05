//! A unique era, track, and vehicle chart and its leaderboard.
#![allow(
    clippy::struct_field_names,
    reason = "entity fields mirror database column names"
)]
use crate::milliseconds::Milliseconds;
use crate::models::{Hotlap, Player, hotlap::SteeringInput, player::Entity as PlayerEntity};
use crate::ordering::Ordering;
use crate::track_id::TrackId;
use crate::vehicle_id::VehicleId;
use celes::Country;
use sea_orm::{
    AccessMode, ActiveEnum, DatabaseConnection, DbBackend, FromQueryResult, IsolationLevel,
    LoaderTrait, Statement, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use sea_orm::entity::prelude::*;

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "chart")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub era_id: i64,
    pub track_id: TrackId,
    pub vehicle_id: VehicleId,
    #[sea_orm(belongs_to, from = "era_id", to = "id", on_delete = "Cascade")]
    pub era: BelongsTo<crate::models::era::Entity>,
    #[sea_orm(belongs_to, from = "track_id", to = "id")]
    pub track: BelongsTo<crate::models::track::Entity>,
    #[sea_orm(belongs_to, from = "vehicle_id", to = "id")]
    pub vehicle: BelongsTo<crate::models::vehicle::Entity>,
    #[sea_orm(has_many)]
    pub memberships: HasMany<crate::models::ranking_chart_membership::Entity>,
    #[sea_orm(has_many)]
    pub hotlaps: HasMany<crate::models::hotlap::Entity>,
}

/// One player's fastest valid hotlap for a requested chart and era.
#[derive(Debug, Clone, PartialEq)]
pub struct BestHotlap {
    /// Global chart position, unchanged by filters or pagination.
    pub position: i64,
    pub hotlap: Hotlap,
    pub player: Player,
    pub distance_to_benchmark_ms: Milliseconds,
    pub distance_to_world_record_ms: Milliseconds,
}

/// Filters applied to the chart's current personal bests.
pub struct BestHotlapFilters {
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
    pub(super) fn sql(self) -> &'static str {
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

impl ActiveModelBehavior for ActiveModel {}

/// A hotlap, chart position, and time gaps. Player details are loaded
/// in one query for the whole page.
#[derive(Debug, FromQueryResult)]
struct BestHotlapRow {
    position: i64,
    #[sea_orm(nested)]
    hotlap: Hotlap,
    distance_to_benchmark_ms: Milliseconds,
    distance_to_world_record_ms: Milliseconds,
}

/// SQL for a percentage benchmark, rounded to the nearest millisecond.
/// Chart leaderboards use 103%; aggregate rankings supply their configured percentage.
pub(crate) fn benchmark_sql(record_expression: &str, percent: i32) -> String {
    format!("(({record_expression} * {percent} + 50) / 100)")
}

const CHART_FROM: &str = r"
FROM hotlap_personal_best pb
JOIN hotlap ON hotlap.id = pb.hotlap_id
JOIN player ON player.id = pb.player_id
";
const CHART_FILTER: &str = r"
WHERE pb.chart_id = $1
  AND ($2::TEXT IS NULL OR hotlap.steering = $2)
  AND ($3::TEXT IS NULL OR player.country_code = $3)
";

/// Lists current personal bests for one chart and era.
impl Model {
    pub async fn leaderboard(
        &self,
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
        let result = list_best_in_snapshot(&transaction, self, filters, page).await?;
        transaction.commit().await?;
        Ok(result)
    }
}

async fn list_best_in_snapshot(
    database: &impl ConnectionTrait,
    chart: &Model,
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
    WHERE pb.chart_id = $1
      AND pb.position = 1
) record
{CHART_FILTER}
ORDER BY {column} {order}, pb.position ASC
LIMIT $4 OFFSET $5
",
        benchmark = benchmark_sql("record.lap_time_ms", 103),
        column = page.column.sql(),
        order = page.order.sql(),
    );

    // Shared filter parameters for both statements.
    let mut values: Vec<sea_orm::Value> = vec![
        chart.id.into(),
        filters.controller.map(SteeringInput::into_value).into(),
        filters
            .country
            .map(|country| country.alpha2.to_owned())
            .into(),
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

/// Shared by chart, era rebuilds, and SQLx imports. The caller holds the
/// chart lock or exclusive catalogue lock until commit.
pub(crate) const RERANK_SQL: &str = r#"
WITH positions AS (
    SELECT pb.hotlap_id,
        ROW_NUMBER() OVER (
            PARTITION BY pb.chart_id
            ORDER BY hotlap.lap_time_ms, hotlap.created_at, player.lfs_username COLLATE "C"
        ) AS position
    FROM hotlap_personal_best pb
    JOIN hotlap ON hotlap.id = pb.hotlap_id
    JOIN player ON player.id = pb.player_id
    JOIN chart ON chart.id = pb.chart_id
    WHERE chart.era_id = $1 AND ($2::BIGINT IS NULL OR chart.id = $2)
)
UPDATE hotlap_personal_best pb
SET position = positions.position
FROM positions
WHERE pb.hotlap_id = positions.hotlap_id
  AND pb.position IS DISTINCT FROM positions.position
"#;

impl Model {
    pub(crate) async fn find_combination(
        database: &impl ConnectionTrait,
        era_id: i64,
        track: &str,
        vehicle: &str,
    ) -> Result<Option<Self>, DbErr> {
        Entity::find()
            .filter(Column::EraId.eq(era_id))
            .filter(Column::TrackId.eq(track))
            .filter(Column::VehicleId.eq(vehicle))
            .one(database)
            .await
    }

    pub(crate) async fn is_available(
        &self,
        database: &impl ConnectionTrait,
    ) -> Result<bool, DbErr> {
        use crate::models::vehicle;
        Ok(self
            .find_related(vehicle::Entity)
            .filter(vehicle::Column::Available.eq(true))
            .one(database)
            .await?
            .is_some())
    }

    /// Rankings selecting this chart, in their public presentation order.
    pub(crate) fn rankings(&self) -> sea_orm::Select<crate::models::ranking::Entity> {
        use crate::models::{ranking, ranking_chart_membership as membership};
        use sea_orm::{QueryOrder, QuerySelect, QueryTrait};
        ranking::Entity::find()
            .filter(
                ranking::Column::Id.in_subquery(
                    membership::Entity::find()
                        .filter(membership::Column::ChartId.eq(self.id))
                        .select_only()
                        .column(membership::Column::RankingId)
                        .into_query(),
                ),
            )
            .order_by_asc(ranking::Column::Position)
            .order_by_asc(ranking::Column::Slug)
    }

    /// Acquire after catalogue admission and hotlap locking, before PB changes.
    pub(crate) async fn lock(&self, database: &impl ConnectionTrait) -> Result<(), DbErr> {
        database
            .query_one_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "SELECT pg_advisory_xact_lock(hashtextextended($1, 0))",
                [format!("hotlap_chart:{}", self.id).into()],
            ))
            .await?;
        Ok(())
    }

    pub(crate) async fn rerank_personal_bests(
        &self,
        database: &impl ConnectionTrait,
    ) -> Result<(), DbErr> {
        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                RERANK_SQL,
                [self.era_id.into(), self.id.into()],
            ))
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Hotlap, hotlap::Entity as HotlapEntity};
    use sea_orm::SqlxPostgresConnector;

    #[sqlx::test]
    #[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
    async fn publication_and_deletion_keep_chart_best_and_positions_consistent(
        pool: sqlx::PgPool,
    ) -> anyhow::Result<()> {
        sqlx::raw_sql(include_str!("../services/validate_hotlap/fixtures.sql"))
            .execute(&pool)
            .await?;
        let database = SqlxPostgresConnector::from_sqlx_postgres_pool(pool.clone());
        let original = HotlapEntity::find().one(&database).await?.unwrap();
        let (published, _) =
            Hotlap::validate_owned_for_testing(&database, original.id, original.player_id)
                .await?
                .unwrap();
        let chart = Entity::find_by_id(published.chart_id)
            .one(&database)
            .await?
            .unwrap();
        let (entries, total) = chart
            .leaderboard(
                &database,
                BestHotlapFilters {
                    country: None,
                    controller: None,
                },
                BestHotlapPage {
                    offset: 0,
                    limit: 10,
                    column: BestHotlapColumn::Rank,
                    order: crate::ordering::Ordering::Asc,
                },
            )
            .await?;
        assert_eq!(total, 1);
        assert_eq!(entries[0].hotlap.id, published.id);
        assert_eq!(entries[0].position, 1);
        assert_eq!(entries[0].distance_to_world_record_ms, Milliseconds::ZERO);

        let second: i64 = sqlx::query_scalar("INSERT INTO hotlap (
            player_id, era_id, chart_id, track, vehicle, raw_vehicle_name, lap_time_ms,
            split_1_ms, split_2_ms, split_3_ms, split_4_ms, original_filename, spr_object_key,
            source, fingerprint, steering, player_flags, game_version, state)
            SELECT player_id, era_id, chart_id, track, vehicle, raw_vehicle_name, lap_time_ms - 1000,
            0,0,0,0, 'second.spr', 'second.spr', source, repeat('b',64), steering, player_flags, game_version, 'pending'
            FROM hotlap WHERE id = $1 RETURNING id").bind(published.id).fetch_one(&pool).await?;
        Hotlap::validate_owned_for_testing(&database, second, published.player_id).await?;
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT hotlap_id FROM hotlap_personal_best")
                .fetch_one(&pool)
                .await?,
            second
        );
        Hotlap::delete_owned(&database, second, published.player_id)
            .await?
            .unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT hotlap_id FROM hotlap_personal_best")
                .fetch_one(&pool)
                .await?,
            published.id
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT position FROM hotlap_personal_best")
                .fetch_one(&pool)
                .await?,
            1
        );
        // The classification columns must agree with the referenced chart.
        assert!(
            sqlx::query("UPDATE hotlap SET vehicle = 'XRG'")
                .execute(&pool)
                .await
                .is_err()
        );
        Ok(())
    }
}
