//! Named predicates over the `personal_access_token` table.

use sea_orm::{ColumnTrait, DeleteMany, QueryFilter, Select, UpdateMany};

use super::{PersonalAccessTokenColumn as Column, PersonalAccessTokenEntity as Entity};

/// Database filters for credential queries.
pub(crate) trait PersonalAccessTokenFilter: QueryFilter + Sized {
    fn owned_by(self, player_id: i64) -> Self {
        self.filter(Column::PlayerId.eq(player_id))
    }

    fn with_id(self, token_id: i64) -> Self {
        self.filter(Column::Id.eq(token_id))
    }

    /// Not revoked. Says nothing about expiry.
    fn live(self) -> Self {
        self.filter(Column::RevokedAt.is_null())
    }

    /// Neither revoked nor expired: the only state that authenticates.
    fn usable_at(self, now: time::OffsetDateTime) -> Self {
        self.live().filter(Column::ExpiresAt.gt(now))
    }
}

impl PersonalAccessTokenFilter for Select<Entity> {}
impl PersonalAccessTokenFilter for UpdateMany<Entity> {}
impl PersonalAccessTokenFilter for DeleteMany<Entity> {}
