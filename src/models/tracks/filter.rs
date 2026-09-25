//! Named predicates over the `track` table.

use sea_orm::{ColumnTrait, QueryFilter, QueryOrder, Select};

use super::{TrackColumn, TrackEntity};

/// Database filters for track queries.
pub(crate) trait TrackFilter: QueryFilter + Sized {
    /// Excludes open configurations, which cannot host a ranked hotlap.
    ///
    /// The same rule `HotlapRankable for Track` applies in memory, expressed
    /// against the stored column so a whole catalogue can be narrowed in SQL.
    fn closed_circuit(self) -> Self {
        self.filter(TrackColumn::OpenConfiguration.eq(false))
    }
}

impl TrackFilter for Select<TrackEntity> {}

/// Canonical presentation ordering for track catalogue queries.
pub(crate) trait TrackOrder: QueryOrder + Sized {
    fn in_catalogue_order(self) -> Self {
        self.order_by_asc(TrackColumn::Sequence)
            .order_by_asc(TrackColumn::Id)
    }
}

impl TrackOrder for Select<TrackEntity> {}
