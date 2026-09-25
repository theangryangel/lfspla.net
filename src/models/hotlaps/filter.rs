//! Named predicates over the `hotlap` table.
//!
//! Filters only: `QueryOrder` is not implemented by every builder this trait
//! covers, so ordering stays at the call site.

use sea_orm::{ColumnTrait, QueryFilter, Select, UpdateMany};

use super::{HotlapColumn, HotlapEntity, HotlapState};

/// The `source` value written by the upload path.
///
/// Imported LFS World laps carry a different source and must stay out of
/// every upload-facing read, so the literal is spelled once.
pub(crate) const SOURCE_UPLOAD: &str = "upload";

/// Uploads still occupying one of a player's queue slots.
const OUTSTANDING_STATES: [HotlapState; 1] = [HotlapState::Pending];

/// Database filters for hotlap queries.
///
/// Deliberately implemented per builder rather than blanket over
/// `QueryFilter`: a blanket impl would offer `uploads()` on a player query and
/// silently emit `WHERE hotlap.source = 'upload'` against the wrong table.
pub(crate) trait HotlapFilter: QueryFilter + Sized {
    /// Player uploads, excluding imported laps.
    fn uploads(self) -> Self {
        self.filter(HotlapColumn::Source.eq(SOURCE_UPLOAD))
    }

    /// Published laps.
    ///
    /// Only reaches query-builder reads. The raw-SQL reads in `activity`,
    /// `leaderboard` and ranking aggregation spell this predicate themselves, and
    /// all public reads also require current combination membership.
    fn valid(self) -> Self {
        self.filter(HotlapColumn::State.eq(HotlapState::Valid))
            .filter(sea_orm::sea_query::Expr::cust("EXISTS (SELECT 1 FROM ranking_chart WHERE era_id = hotlap.era_id AND track_id = hotlap.track AND vehicle_id = hotlap.vehicle)"))
    }

    fn owned_by(self, player_id: i64) -> Self {
        self.filter(HotlapColumn::PlayerId.eq(player_id))
    }

    fn in_era(self, era_id: i64) -> Self {
        self.filter(HotlapColumn::EraId.eq(era_id))
    }

    fn in_state(self, state: HotlapState) -> Self {
        self.filter(HotlapColumn::State.eq(state))
    }

    /// Uploads that have not yet reached a terminal state.
    fn outstanding(self) -> Self {
        self.filter(HotlapColumn::State.is_in(OUTSTANDING_STATES))
    }
}

impl HotlapFilter for Select<HotlapEntity> {}
impl HotlapFilter for UpdateMany<HotlapEntity> {}
