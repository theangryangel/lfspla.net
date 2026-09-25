//! SeaORM entity for era-scoped ranking definitions.
#![allow(
    clippy::struct_field_names,
    reason = "entity fields mirror database column names"
)]

use sea_orm::entity::prelude::*;

use crate::models::rankings::RankingRules;

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
    pub era: BelongsTo<crate::models::eras::Entity>,
    #[sea_orm(has_many)]
    pub charts: HasMany<crate::models::rankings::chart_entity::Entity>,
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
}

impl ActiveModelBehavior for ActiveModel {}
