//! Named predicates over the `ranking` and `ranking_chart` tables.

use sea_orm::{ColumnTrait, QueryFilter, QueryOrder, Select};

use super::{RankingChartColumn, RankingChartEntity, RankingColumn, RankingEntity};

/// Predicates over era-scoped ranking definitions.
pub(crate) trait RankingFilter: QueryFilter + Sized {
    fn in_era(self, era_id: i64) -> Self {
        self.filter(RankingColumn::EraId.eq(era_id))
    }

    /// Narrows to one ranking identifier.
    ///
    /// Only unique within an era, so this is a half key: pair it with
    /// [`in_era`](RankingFilter::in_era) to address a single row.
    fn with_slug(self, ranking_slug: &str) -> Self {
        self.filter(RankingColumn::Slug.eq(ranking_slug))
    }
}

impl RankingFilter for Select<RankingEntity> {}

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
