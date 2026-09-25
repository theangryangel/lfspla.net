//! Configurable rules for aggregate rankings.

use sea_orm::DatabaseConnection;
use validator::{Validate, ValidationError};

use crate::models::country::CountryCode;

pub(crate) use super::aggregate::benchmark_sql;

/// Numeric configuration for the one aggregate ranking system.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    serde::Deserialize,
    serde::Serialize,
    utoipa::ToSchema,
    Validate,
)]
#[validate(schema(function = "validate_nation_driver_limit"))]
#[serde(deny_unknown_fields)]
pub(crate) struct RankingRules {
    #[validate(range(min = 1))]
    pub(crate) benchmark_percent: i32,
    #[validate(range(min = 1))]
    pub(crate) nation_max_points: i32,
    #[validate(range(min = 1))]
    pub(crate) nation_driver_limit: i32,
}

fn validate_nation_driver_limit(rules: &RankingRules) -> Result<(), ValidationError> {
    if rules.nation_driver_limit > rules.nation_max_points {
        return Err(ValidationError::new(
            "nation_driver_limit_exceeds_max_points",
        ));
    }
    Ok(())
}

pub(super) const DEFAULT_RANKING_RULES: RankingRules = RankingRules {
    benchmark_percent: 103,
    nation_max_points: 10,
    nation_driver_limit: 3,
};

/// Calculates personal standings using one persisted ranking definition.
pub(crate) async fn personal(
    database: &impl sea_orm::ConnectionTrait,
    rules: RankingRules,
    era_id: i64,
    ranking_id: i64,
    limit: u64,
) -> Result<Vec<PersonalRankingRow>, sea_orm::DbErr> {
    super::aggregate::personal(database, rules, era_id, ranking_id, limit).await
}

/// Calculates national standings using one persisted ranking definition.
pub(crate) async fn nations(
    database: &DatabaseConnection,
    rules: RankingRules,
    era_id: i64,
    ranking_id: i64,
    limit: u64,
) -> Result<Vec<NationRankingRow>, sea_orm::DbErr> {
    super::aggregate::nations(database, rules, era_id, ranking_id, limit).await
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
    pub completed_charts: i64,
    pub handicap_ms: i64,
}

/// Aggregated national result produced by the ranking calculation.
#[derive(Debug, Clone, PartialEq, sea_orm::FromQueryResult)]
pub(crate) struct NationRankingRow {
    pub position: i64,
    /// Validated as the row is read. Not optional: the query that produces
    /// these rows excludes players with no nation.
    pub country_code: CountryCode,
    pub points: i64,
    pub handicap_ms: i64,
    pub contributing_laps: i64,
    pub contributing_charts: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranking_rules_validate_positive_values_and_the_nation_relationship() {
        assert!(DEFAULT_RANKING_RULES.validate().is_ok());
        assert!(
            RankingRules {
                benchmark_percent: 0,
                ..DEFAULT_RANKING_RULES
            }
            .validate()
            .is_err()
        );
        assert!(
            RankingRules {
                nation_driver_limit: 11,
                ..DEFAULT_RANKING_RULES
            }
            .validate()
            .is_err()
        );
    }
}
