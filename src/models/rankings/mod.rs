//! Ranking definitions, rules, calculations, and persistence entities.

mod aggregate;
pub(crate) mod chart_entity;
mod charts;
pub(crate) use charts::with_charts;
mod definition;
pub(crate) mod entity;
mod filter;
mod rules;
mod standings;

pub use chart_entity::{
    ActiveModel as RankingChartMutation, Column as RankingChartColumn,
    Entity as RankingChartEntity, Model as RankingChartModel,
};
pub(crate) use entity::*;
pub use entity::{
    ActiveModel as RankingMutation, Column as RankingColumn, Entity as RankingEntity,
    Model as RankingRow,
};

pub(crate) use filter::{RankingChartOrder, RankingFilter};

pub use definition::{RankingBadgeDefinition, RankingBadgeQualification};
pub(crate) use rules::{
    NationRankingRow, PersonalRankingRow, RankingRules, benchmark_sql, nations, personal,
};
pub(crate) use standings::LoadedRanking;

pub(crate) use aggregate::{NationContribution, PersonalChartBest, PersonalRankingProgress};
