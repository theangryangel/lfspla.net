//! Named predicates over the `player_era_badges` table.

use sea_orm::{ColumnTrait, DeleteMany, QueryFilter, Select};

use super::{PlayerEraBadgesColumn as Column, PlayerEraBadgesEntity as Entity};

/// Database filters for badge queries.
pub(crate) trait BadgeFilter: QueryFilter + Sized {
    fn owned_by(self, player_id: i64) -> Self {
        self.filter(Column::PlayerId.eq(player_id))
    }

    fn owned_by_any(self, player_ids: Vec<i64>) -> Self {
        self.filter(Column::PlayerId.is_in(player_ids))
    }

    fn in_era(self, era_id: i64) -> Self {
        self.filter(Column::EraId.eq(era_id))
    }
}

impl BadgeFilter for Select<Entity> {}
impl BadgeFilter for DeleteMany<Entity> {}
