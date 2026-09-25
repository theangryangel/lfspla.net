//! Badge definitions attached to stored rankings.

use std::num::NonZeroU64;

use serde::{Deserialize, Serialize};
use validator::Validate;

/// Presentation and eligibility rules for a badge owned by one era ranking.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Validate)]
#[serde(deny_unknown_fields)]
pub struct RankingBadgeDefinition {
    /// Compact text displayed beside a player's name.
    #[validate(custom(function = "crate::validate::validate_non_blank"))]
    pub label: String,
    /// Which personal-ranking entries receive the badge.
    pub qualification: RankingBadgeQualification,
    /// An additional award for completing every chart, regardless of position.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[validate(nested)]
    pub completion: Option<RankingCompletionBadgeDefinition>,
}

/// Optional completion award alongside the ranking's primary badge.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Validate)]
#[serde(deny_unknown_fields)]
pub struct RankingCompletionBadgeDefinition {
    #[validate(custom(function = "crate::validate::validate_non_blank"))]
    pub label: String,
}

impl RankingBadgeDefinition {
    pub(crate) fn result_limit(&self) -> u64 {
        if self.completion.is_some() {
            u64::MAX
        } else {
            self.qualification.result_limit()
        }
    }
}

/// Eligibility strategies supported by ranking-derived badges.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum RankingBadgeQualification {
    /// Leading players present in the personal ranking, optionally bounded.
    Ranked {
        /// Maximum number of leading entries awarded the badge, if bounded.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        limit: Option<NonZeroU64>,
    },
    /// Only players with a result on every chart selected by the ranking.
    Complete,
}

impl RankingBadgeQualification {
    /// Maximum number of personal-ranking rows needed to award this badge.
    pub(crate) const fn result_limit(self) -> u64 {
        match self {
            Self::Ranked { limit: Some(limit) } => limit.get(),
            Self::Ranked { limit: None } | Self::Complete => u64::MAX,
        }
    }
}
