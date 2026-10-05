//! Configurable rules for aggregate rankings.

use validator::{Validate, ValidationError};

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

#[cfg(test)]
pub(super) const DEFAULT_RANKING_RULES: RankingRules = RankingRules {
    benchmark_percent: 103,
    nation_max_points: 10,
    nation_driver_limit: 3,
};

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
