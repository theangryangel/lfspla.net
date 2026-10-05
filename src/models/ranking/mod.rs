//! Ranking definitions, rules, calculations, and persistence entities.
#![allow(
    clippy::struct_field_names,
    reason = "entity fields mirror database column names"
)]
use sea_orm::entity::prelude::*;

use crate::models::chart::benchmark_sql;
use crate::models::{Hotlap, Player, badge::PlayerBadge};
use crate::{country::CountryCode, milliseconds::Milliseconds};
use lfsplanet_flags::FlagCode;
use sea_orm::{
    ColumnTrait, DatabaseConnection, DbBackend, FromQueryResult, PaginatorTrait, QueryFilter,
    Select, Statement,
};
const STANDINGS_SIZE: u64 = 100;

mod badge;
mod charts;
mod rules;

pub use badge::RankingBadgeDefinition;
pub use badge::RankingBadgeQualification;
pub(crate) use charts::RankingWithCharts;
pub(crate) use rules::RankingRules;

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "ranking")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub era_id: i64,
    /// Stable YAML/API identity, unique within its era.
    pub slug: crate::ranking_slug::RankingSlug,
    pub position: i32,
    pub title: String,
    pub description: String,
    pub benchmark_percent: i32,
    pub nation_max_points: i32,
    pub nation_driver_limit: i32,
    pub badge: Option<Json>,
    #[sea_orm(
        belongs_to,
        from = "era_id",
        to = "id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    pub era: BelongsTo<crate::models::era::Entity>,
    #[sea_orm(has_many)]
    pub memberships: HasMany<crate::models::ranking_chart_membership::Entity>,
}

impl Model {
    pub(crate) async fn find(
        database: &impl ConnectionTrait,
        era_id: i64,
        slug: &str,
    ) -> Result<Option<Self>, DbErr> {
        Entity::find()
            .in_era(era_id)
            .with_slug(slug)
            .one(database)
            .await
    }

    pub(crate) async fn chart_count(
        &self,
        database: &impl ConnectionTrait,
    ) -> Result<usize, DbErr> {
        let count = self
            .find_related(crate::models::ranking_chart_membership::Entity)
            .count(database)
            .await?;
        usize::try_from(count).map_err(|error| DbErr::Type(error.to_string()))
    }

    /// Returns the calculation rules persisted with this ranking.
    pub(crate) const fn rules(&self) -> RankingRules {
        RankingRules {
            benchmark_percent: self.benchmark_percent,
            nation_max_points: self.nation_max_points,
            nation_driver_limit: self.nation_driver_limit,
        }
    }

    pub(crate) async fn personal_scores(
        &self,
        database: &impl sea_orm::ConnectionTrait,
        limit: u64,
    ) -> Result<Vec<PersonalRankingRow>, DbErr> {
        let rules = self.rules();
        let sql = format!(
            r"
WITH eligible AS (
    SELECT
        ranking_chart_membership.chart_id,
        hotlap_personal_best.position AS chart_position,
        hotlap.player_id,
        hotlap.lap_time_ms
    FROM ranking_chart_membership
    JOIN hotlap_personal_best
      ON hotlap_personal_best.chart_id = ranking_chart_membership.chart_id
    JOIN hotlap ON hotlap.id = hotlap_personal_best.hotlap_id
    WHERE ranking_chart_membership.ranking_id = $1
),
records AS (
    SELECT chart_id, lap_time_ms AS record_ms
    FROM eligible
    WHERE chart_position = 1
),
totals AS (
    SELECT
        eligible.player_id,
        COUNT(*)::BIGINT AS completed_charts,
        SUM(
            eligible.lap_time_ms - {benchmark}
        )::BIGINT AS handicap_ms
    FROM eligible
    JOIN records USING (chart_id)
    GROUP BY eligible.player_id
),
ranked AS (
    SELECT
        RANK() OVER (
            ORDER BY totals.completed_charts DESC,
                     totals.handicap_ms
        )::BIGINT AS position,
        totals.player_id,
        player.lfs_username,
        player.display_name,
        player.country_code,
        player.flag_code,
        totals.completed_charts,
        totals.handicap_ms
    FROM totals
    JOIN player ON player.id = totals.player_id
)
SELECT *
FROM ranked
ORDER BY position, player_id
LIMIT $2
",
            benchmark = benchmark_sql("records.record_ms", rules.benchmark_percent),
        );

        PersonalRankingRow::find_by_statement(Statement::from_sql_and_values(
            DbBackend::Postgres,
            sql,
            [self.id.into(), limit_parameter(limit).into()],
        ))
        .all(database)
        .await
    }
}

impl ActiveModelBehavior for ActiveModel {}

/// Predicates over era-scoped ranking definitions.
pub(crate) trait RankingFilter: QueryFilter + Sized {
    fn in_era(self, era_id: i64) -> Self {
        self.filter(Column::EraId.eq(era_id))
    }

    /// Narrows to one ranking identifier.
    ///
    /// Only unique within an era, so this is a half key: pair it with
    /// [`in_era`](RankingFilter::in_era) to address a single row.
    fn with_slug(self, ranking_slug: &str) -> Self {
        self.filter(Column::Slug.eq(ranking_slug))
    }
}

impl RankingFilter for Select<Entity> {}

/// Completion totals for every ranking in an era, for one player.
#[derive(Debug, FromQueryResult)]
pub(crate) struct PersonalRankingProgress {
    pub ranking_id: i64,
    pub total_combinations: i64,
    pub completed_combinations: i64,
}

fn limit_parameter(limit: u64) -> i64 {
    i64::try_from(limit).unwrap_or(i64::MAX)
}

/// Shared scoring eligibility keeps the breakdown and national totals in agreement.
fn nation_contributors_sql(rules: RankingRules) -> String {
    format!(
        r"
WITH eligible AS (
    SELECT
        ranking_chart_membership.chart_id,
        hotlap_personal_best.position AS chart_position,
        hotlap.player_id,
        player.country_code,
        hotlap.lap_time_ms
    FROM ranking_chart_membership
    JOIN hotlap_personal_best
      ON hotlap_personal_best.chart_id = ranking_chart_membership.chart_id
    JOIN hotlap ON hotlap.id = hotlap_personal_best.hotlap_id
    JOIN player ON player.id = hotlap.player_id
    WHERE ranking_chart_membership.ranking_id = $1
),
records AS (
    SELECT chart_id, lap_time_ms AS record_ms
    FROM eligible
    WHERE chart_position = 1
),
scoring_laps AS (
    SELECT
        eligible.*,
        ROW_NUMBER() OVER (
            PARTITION BY chart_id, country_code
            ORDER BY chart_position
        ) AS country_position
    FROM eligible
    WHERE chart_position <= {nation_max_points}
      AND country_code IS NOT NULL
),
contributors AS (
    SELECT
        scoring_laps.chart_id,
        scoring_laps.player_id,
        scoring_laps.country_code,
        scoring_laps.chart_position,
        scoring_laps.lap_time_ms,
        records.record_ms
    FROM scoring_laps
    JOIN records USING (chart_id)
    WHERE scoring_laps.country_position <= {nation_driver_limit}
)
",
        nation_max_points = rules.nation_max_points,
        nation_driver_limit = rules.nation_driver_limit
    )
}

#[derive(Debug, sea_orm::FromQueryResult, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct NationContribution {
    pub player_id: i64,
    pub lfs_username: String,
    pub display_name: String,
    pub points: i64,
    pub contributing_charts: i64,
    pub handicap_ms: Milliseconds,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranking_rules_remain_planning_constants_not_bind_parameters() {
        assert_eq!(limit_parameter(100), 100);
        assert_eq!(
            benchmark_sql("record_ms", 107),
            "((record_ms * 107 + 50) / 100)"
        );
    }

    #[sqlx::test]
    #[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
    async fn configured_benchmark_agrees_between_chart_details_and_all_standings(
        pool: sqlx::PgPool,
    ) -> anyhow::Result<()> {
        sqlx::raw_sql(include_str!("../../services/validate_hotlap/fixtures.sql"))
            .execute(&pool)
            .await?;
        let database = sea_orm::SqlxPostgresConnector::from_sqlx_postgres_pool(pool.clone());
        let hotlap = crate::models::hotlap::Entity::find()
            .one(&database)
            .await?
            .unwrap();
        crate::models::Hotlap::validate_owned_for_testing(&database, hotlap.id, hotlap.player_id)
            .await?;
        sqlx::query("UPDATE player SET country_code = 'GB'")
            .execute(&pool)
            .await?;
        let player = crate::models::player::Entity::find_by_id(hotlap.player_id)
            .one(&database)
            .await?
            .unwrap();
        for (percent, expected_gap) in [(107, -4200), (109, -5400)] {
            sqlx::query("UPDATE ranking SET benchmark_percent = $1")
                .bind(percent)
                .execute(&pool)
                .await?;
            let ranking = Model::find(&database, hotlap.era_id, "all").await?.unwrap();
            assert_eq!(ranking.chart_count(&database).await?, 1);
            let bests = ranking
                .list_personal_chart_bests(&database, &player)
                .await?;
            assert_eq!(bests.len(), 1);
            assert_eq!(bests[0].distance_to_benchmark_ms.as_millis(), expected_gap);
            assert_eq!(bests[0].distance_to_world_record_ms, Milliseconds::ZERO);
            let personal = ranking.personal(&database).await?;
            assert_eq!(personal[0].row.completed_charts, 1);
            assert_eq!(personal[0].row.handicap_ms.as_millis(), expected_gap);
            let nations = ranking.nations(&database).await?;
            assert_eq!(nations[0].contributing_charts, 1);
            assert_eq!(nations[0].points, 10);
            assert_eq!(nations[0].handicap_ms.as_millis(), expected_gap);
            let contributors = ranking.list_nation_contributions(&database, "GB").await?;
            assert_eq!(contributors[0].handicap_ms.as_millis(), expected_gap);
        }
        Ok(())
    }
}

/// Aggregated personal result produced by the ranking calculation.
#[derive(Debug, Clone, PartialEq, sea_orm::FromQueryResult)]
pub(crate) struct PersonalRankingRow {
    pub position: i64,
    pub player_id: i64,
    pub lfs_username: String,
    pub display_name: String,
    /// Validated as the row is read, like every other country column.
    pub country_code: Option<CountryCode>,
    pub flag_code: Option<FlagCode>,
    pub completed_charts: i64,
    pub handicap_ms: Milliseconds,
}

/// Aggregated national result produced by the ranking calculation.
#[derive(Debug, Clone, PartialEq, sea_orm::FromQueryResult)]
pub(crate) struct NationRankingRow {
    pub position: i64,
    /// Validated as the row is read. Not optional: the query that produces
    /// these rows excludes players with no nation.
    pub country_code: CountryCode,
    pub points: i64,
    pub handicap_ms: Milliseconds,
    pub contributing_laps: i64,
    pub contributing_charts: i64,
}

/// A player's current personal best for one chart selected by a ranking.
#[derive(Debug, FromQueryResult)]
pub(crate) struct PersonalChartBest {
    #[sea_orm(nested)]
    pub hotlap: Hotlap,
    pub position: i64,
    pub distance_to_benchmark_ms: Milliseconds,
    pub distance_to_world_record_ms: Milliseconds,
}

/// A personal ranking row with its presentation badges.
pub(crate) struct PersonalStanding {
    pub(crate) row: PersonalRankingRow,
    pub(crate) badges: Vec<PlayerBadge>,
}

impl Model {
    pub(crate) async fn personal(
        &self,
        database: &DatabaseConnection,
    ) -> Result<Vec<PersonalStanding>, sea_orm::DbErr> {
        let rows = self.personal_scores(database, STANDINGS_SIZE).await?;
        let mut badges = crate::models::badge::Model::list_for_players(
            database,
            self.era_id,
            rows.iter().map(|row| row.player_id),
        )
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| PersonalStanding {
                badges: badges.remove(&row.player_id).unwrap_or_default(),
                row,
            })
            .collect())
    }

    pub(crate) async fn nations(
        &self,
        database: &DatabaseConnection,
    ) -> Result<Vec<NationRankingRow>, sea_orm::DbErr> {
        let rules = self.rules();
        // These values are constrained integer columns, so rendering them cannot
        // introduce SQL. Keeping the window limits literal is deliberate:
        // PostgreSQL can use bounded WindowAgg plans for this query.
        let sql = format!(
            r"
{contributors},
totals AS (
    SELECT
        country_code,
        SUM({nation_score_base} - chart_position)::BIGINT AS points,
        SUM(
            lap_time_ms - {benchmark}
        )::BIGINT AS handicap_ms,
        COUNT(*)::BIGINT AS contributing_laps,
        COUNT(DISTINCT chart_id)::BIGINT AS contributing_charts
    FROM contributors
    GROUP BY country_code
),
ranked AS (
    SELECT
        RANK() OVER (
            ORDER BY totals.points DESC, totals.handicap_ms
        )::BIGINT AS position,
        totals.country_code,
        totals.points,
        totals.handicap_ms,
        totals.contributing_laps,
        totals.contributing_charts
    FROM totals
)
SELECT *
FROM ranked
ORDER BY position, country_code
LIMIT $2
",
            contributors = nation_contributors_sql(rules),
            benchmark = benchmark_sql("record_ms", rules.benchmark_percent),
            nation_score_base = rules.nation_max_points + 1,
        );

        NationRankingRow::find_by_statement(Statement::from_sql_and_values(
            DbBackend::Postgres,
            sql,
            [self.id.into(), (STANDINGS_SIZE as i64).into()],
        ))
        .all(database)
        .await
    }

    /// Lists this player's best lap for each chart in this ranking.
    pub(crate) async fn list_personal_chart_bests(
        &self,
        database: &DatabaseConnection,
        player: &Player,
    ) -> Result<Vec<PersonalChartBest>, sea_orm::DbErr> {
        let benchmark = benchmark_sql("record.lap_time_ms", self.benchmark_percent);
        PersonalChartBest::find_by_statement(Statement::from_sql_and_values(
            DbBackend::Postgres,
            format!(
                r"
SELECT hotlap.*,
       hotlap_personal_best.position,
       hotlap.lap_time_ms - {benchmark} AS distance_to_benchmark_ms,
       hotlap.lap_time_ms - record.lap_time_ms AS distance_to_world_record_ms
FROM ranking_chart_membership
JOIN hotlap_personal_best
  ON hotlap_personal_best.chart_id = ranking_chart_membership.chart_id
JOIN hotlap ON hotlap.id = hotlap_personal_best.hotlap_id
JOIN hotlap_personal_best AS record_best
  ON record_best.chart_id = ranking_chart_membership.chart_id
 AND record_best.position = 1
JOIN hotlap AS record ON record.id = record_best.hotlap_id
WHERE ranking_chart_membership.ranking_id = $1
  AND hotlap_personal_best.player_id = $2
ORDER BY ranking_chart_membership.position
",
            ),
            [self.id.into(), player.id.into()],
        ))
        .all(database)
        .await
    }

    /// Lists the players whose laps contribute to this country's score.
    pub(crate) async fn list_nation_contributions(
        &self,
        database: &DatabaseConnection,
        country: &str,
    ) -> Result<Vec<NationContribution>, sea_orm::DbErr> {
        let rules = self.rules();
        let sql = format!(
            r#"
{contributors}
SELECT contributors.player_id, player.lfs_username, player.display_name,
       SUM({score_base} - chart_position)::BIGINT AS points,
       COUNT(DISTINCT chart_id)::BIGINT AS contributing_charts,
       SUM(lap_time_ms - {benchmark})::BIGINT AS handicap_ms
FROM contributors
JOIN player ON player.id = contributors.player_id
WHERE contributors.country_code = $2
GROUP BY contributors.player_id, player.lfs_username, player.display_name
ORDER BY points DESC, handicap_ms, player.lfs_username COLLATE "C"
"#,
            contributors = nation_contributors_sql(rules),
            score_base = rules.nation_max_points + 1,
            benchmark = benchmark_sql("record_ms", rules.benchmark_percent),
        );
        NationContribution::find_by_statement(Statement::from_sql_and_values(
            DbBackend::Postgres,
            sql,
            [self.id.into(), country.into()],
        ))
        .all(database)
        .await
    }
}
