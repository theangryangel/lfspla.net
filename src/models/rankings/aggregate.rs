//! Aggregate personal and nation ranking calculations.

use sea_orm::{DatabaseConnection, DbBackend, FromQueryResult, Statement};

use super::rules::{DEFAULT_RANKING_RULES, NationRankingRow, PersonalRankingRow, RankingRules};
use crate::models::hotlaps::HotlapModel;

/// SQL for the benchmark time, rounded to the nearest millisecond.
/// Shared by personal standings, national standings, and chart time gaps.
pub(crate) fn benchmark_sql(record_expression: &str) -> String {
    chart_benchmark_sql(record_expression, DEFAULT_RANKING_RULES.benchmark_percent)
}

fn chart_benchmark_sql(record_expression: &str, percent: i32) -> String {
    format!("(({record_expression} * {percent} + 50) / 100)")
}

pub(super) async fn personal(
    database: &impl sea_orm::ConnectionTrait,
    rules: RankingRules,
    era_id: i64,
    ranking_id: i64,
    limit: u64,
) -> Result<Vec<PersonalRankingRow>, sea_orm::DbErr> {
    let sql = format!(
        r"
WITH eligible AS (
    SELECT
        ranking_chart.position AS chart_index,
        hotlap_personal_best.position AS chart_position,
        hotlap.player_id,
        hotlap.lap_time_ms
    FROM ranking_chart
    JOIN hotlap_personal_best
      ON hotlap_personal_best.era_id = ranking_chart.era_id
     AND hotlap_personal_best.track = ranking_chart.track_id
     AND hotlap_personal_best.vehicle = ranking_chart.vehicle_id
    JOIN hotlap ON hotlap.id = hotlap_personal_best.hotlap_id
    WHERE ranking_chart.era_id = $1
      AND ranking_chart.ranking_id = $2
),
records AS (
    SELECT chart_index, lap_time_ms AS record_ms
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
    JOIN records USING (chart_index)
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
        totals.completed_charts,
        totals.handicap_ms
    FROM totals
    JOIN player ON player.id = totals.player_id
)
SELECT *
FROM ranked
ORDER BY position, player_id
LIMIT $3
",
        benchmark = chart_benchmark_sql("records.record_ms", rules.benchmark_percent),
    );

    PersonalRankingRow::find_by_statement(Statement::from_sql_and_values(
        DbBackend::Postgres,
        sql,
        [
            era_id.into(),
            ranking_id.into(),
            limit_parameter(limit).into(),
        ],
    ))
    .all(database)
    .await
}

/// A player's current personal best for one chart selected by a ranking.
#[derive(Debug, FromQueryResult)]
pub(crate) struct PersonalChartBest {
    pub track: String,
    pub vehicle: String,
    #[sea_orm(nested)]
    pub hotlap: HotlapModel,
    pub position: i64,
    pub distance_to_benchmark_ms: i64,
    pub distance_to_world_record_ms: i64,
}

/// Lists a player's current personal bests for the charts in one ranking.
///
/// `hotlap_personal_best` contains only validated laps, so this is both the
/// fastest relevant lap and the completion source for a ranking chart.
pub(super) async fn personal_chart_bests(
    database: &DatabaseConnection,
    era_id: i64,
    ranking_id: i64,
    player_id: i64,
) -> Result<Vec<PersonalChartBest>, sea_orm::DbErr> {
    let benchmark = benchmark_sql("record.lap_time_ms");
    PersonalChartBest::find_by_statement(Statement::from_sql_and_values(
        DbBackend::Postgres,
        format!(
            r"
SELECT ranking_chart.track_id AS track,
       ranking_chart.vehicle_id AS vehicle,
       hotlap.*,
       hotlap_personal_best.position,
       hotlap.lap_time_ms - {benchmark} AS distance_to_benchmark_ms,
       hotlap.lap_time_ms - record.lap_time_ms AS distance_to_world_record_ms
FROM ranking_chart
JOIN hotlap_personal_best
  ON hotlap_personal_best.era_id = ranking_chart.era_id
 AND hotlap_personal_best.track = ranking_chart.track_id
 AND hotlap_personal_best.vehicle = ranking_chart.vehicle_id
JOIN hotlap ON hotlap.id = hotlap_personal_best.hotlap_id
JOIN hotlap_personal_best AS record_best
  ON record_best.era_id = ranking_chart.era_id
 AND record_best.track = ranking_chart.track_id
 AND record_best.vehicle = ranking_chart.vehicle_id
 AND record_best.position = 1
JOIN hotlap AS record ON record.id = record_best.hotlap_id
WHERE ranking_chart.era_id = $1
  AND ranking_chart.ranking_id = $2
  AND hotlap_personal_best.player_id = $3
ORDER BY ranking_chart.position
",
        ),
        [era_id.into(), ranking_id.into(), player_id.into()],
    ))
    .all(database)
    .await
}

/// Completion totals for every ranking in an era, for one player.
#[derive(Debug, FromQueryResult)]
pub(crate) struct PersonalRankingProgress {
    pub ranking_id: i64,
    pub total_combinations: i64,
    pub completed_combinations: i64,
}

pub(super) async fn nations(
    database: &impl sea_orm::ConnectionTrait,
    rules: RankingRules,
    era_id: i64,
    ranking_id: i64,
    limit: u64,
) -> Result<Vec<NationRankingRow>, sea_orm::DbErr> {
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
        COUNT(DISTINCT chart_index)::BIGINT AS contributing_charts
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
LIMIT $3
",
        contributors = nation_contributors_sql(rules),
        benchmark = chart_benchmark_sql("record_ms", rules.benchmark_percent),
        nation_score_base = rules.nation_max_points + 1,
    );

    NationRankingRow::find_by_statement(Statement::from_sql_and_values(
        DbBackend::Postgres,
        sql,
        [
            era_id.into(),
            ranking_id.into(),
            limit_parameter(limit).into(),
        ],
    ))
    .all(database)
    .await
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
        ranking_chart.position AS chart_index,
        hotlap_personal_best.position AS chart_position,
        hotlap.player_id,
        player.country_code,
        hotlap.lap_time_ms
    FROM ranking_chart
    JOIN hotlap_personal_best
      ON hotlap_personal_best.era_id = ranking_chart.era_id
     AND hotlap_personal_best.track = ranking_chart.track_id
     AND hotlap_personal_best.vehicle = ranking_chart.vehicle_id
    JOIN hotlap ON hotlap.id = hotlap_personal_best.hotlap_id
    JOIN player ON player.id = hotlap.player_id
    WHERE ranking_chart.era_id = $1
      AND ranking_chart.ranking_id = $2
),
records AS (
    SELECT chart_index, lap_time_ms AS record_ms
    FROM eligible
    WHERE chart_position = 1
),
scoring_laps AS (
    SELECT
        eligible.*,
        ROW_NUMBER() OVER (
            PARTITION BY chart_index, country_code
            ORDER BY chart_position
        ) AS country_position
    FROM eligible
    WHERE chart_position <= {nation_max_points}
      AND country_code IS NOT NULL
),
contributors AS (
    SELECT
        scoring_laps.chart_index,
        scoring_laps.player_id,
        scoring_laps.country_code,
        scoring_laps.chart_position,
        scoring_laps.lap_time_ms,
        records.record_ms
    FROM scoring_laps
    JOIN records USING (chart_index)
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
    pub handicap_ms: i64,
}

pub(super) async fn nation_contributions(
    database: &impl sea_orm::ConnectionTrait,
    rules: RankingRules,
    era_id: i64,
    ranking_id: i64,
    country: &str,
) -> Result<Vec<NationContribution>, sea_orm::DbErr> {
    let sql = format!(
        r#"
{contributors}
SELECT contributors.player_id, player.lfs_username, player.display_name,
       SUM({score_base} - chart_position)::BIGINT AS points,
       COUNT(DISTINCT chart_index)::BIGINT AS contributing_charts,
       SUM(lap_time_ms - {benchmark})::BIGINT AS handicap_ms
FROM contributors
JOIN player ON player.id = contributors.player_id
WHERE contributors.country_code = $3
GROUP BY contributors.player_id, player.lfs_username, player.display_name
ORDER BY points DESC, handicap_ms, player.lfs_username COLLATE "C"
"#,
        contributors = nation_contributors_sql(rules),
        score_base = rules.nation_max_points + 1,
        benchmark = chart_benchmark_sql("record_ms", rules.benchmark_percent),
    );
    NationContribution::find_by_statement(Statement::from_sql_and_values(
        DbBackend::Postgres,
        sql,
        [era_id.into(), ranking_id.into(), country.into()],
    ))
    .all(database)
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranking_rules_remain_planning_constants_not_bind_parameters() {
        assert_eq!(limit_parameter(100), 100);
        assert_eq!(
            chart_benchmark_sql("record_ms", 107),
            "((record_ms * 107 + 50) / 100)"
        );
    }
}
