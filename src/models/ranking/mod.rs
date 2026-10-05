//! Ranking definitions, rules, calculations, and persistence entities.
#![allow(
    clippy::struct_field_names,
    reason = "entity fields mirror database column names"
)]
use sea_orm::entity::prelude::*;

use crate::models::ranking_chart::{Column as RankingChartColumn, Entity as RankingChartEntity};

use sea_orm::{ColumnTrait, QueryFilter, QueryOrder, Select};

mod badge;
mod calculations;
mod charts;
mod rules;
mod standings;

pub use badge::RankingBadgeDefinition;
pub use badge::RankingBadgeQualification;
pub(crate) use calculations::NationRankingRow;
pub(crate) use calculations::PersonalRankingRow;
pub(crate) use calculations::benchmark_sql;
pub(crate) use rules::RankingRules;
pub(crate) use standings::RankingWithCharts;

pub(crate) use calculations::NationContribution;
pub(crate) use calculations::PersonalChartBest;
pub(crate) use calculations::PersonalRankingProgress;

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "ranking")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub era_id: i64,
    /// Stable YAML/API identity, unique within its era.
    pub slug: String,
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
    pub charts: HasMany<crate::models::ranking_chart::Entity>,
}

impl Model {
    /// Returns the calculation rules persisted with this ranking.
    pub(crate) const fn rules(&self) -> RankingRules {
        RankingRules {
            benchmark_percent: self.benchmark_percent,
            nation_max_points: self.nation_max_points,
            nation_driver_limit: self.nation_driver_limit,
        }
    }

    pub(crate) async fn personal_scores(
        database: &impl sea_orm::ConnectionTrait,
        rules: RankingRules,
        era_id: i64,
        ranking_id: i64,
        limit: u64,
    ) -> Result<Vec<PersonalRankingRow>, DbErr> {
        calculations::personal(database, rules, era_id, ranking_id, limit).await
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

/// Canonical presentation orderings for ranking chart select queries.
pub(crate) trait RankingChartOrder: QueryOrder + Sized {
    /// Stable lexical order for one chart per track and vehicle pair.
    fn in_combination_order(self) -> Self {
        self.order_by_asc(RankingChartColumn::TrackId)
            .order_by_asc(RankingChartColumn::VehicleId)
    }

    /// The configured order within a ranking definition.
    fn by_position(self) -> Self {
        self.order_by_asc(RankingChartColumn::Position)
    }
}

impl RankingChartOrder for Select<RankingChartEntity> {}
