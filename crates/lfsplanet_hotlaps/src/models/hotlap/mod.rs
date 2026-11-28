//! Ranked-hotlap rules, records, queries, and validation lifecycle.

use lfsplanet_core::milliseconds::Milliseconds;

use crate::{game_version::GameVersionCode, player_flags::PlayerFlagsBits};
use lfsplanet_core::{track_id::TrackId, vehicle_id::VehicleId};

use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, entity::prelude::*};

use sea_orm::{Select, UpdateMany};

use serde::{Deserialize, Serialize};

use utoipa::ToSchema;

mod list;
pub mod pubstat;
pub use list::{
    HotlapListEntry, HotlapListError, HotlapListFilters, HotlapListPage, HotlapPage,
    RankingContribution,
};
mod controls;
mod lifecycle;
pub use lifecycle::{InsertError, NewHotlap, TestValidationError};

pub use controls::*;

/// A recorded lap throughout upload/import and validation.
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "hotlap")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub player_id: i64,
    pub era_id: i64,
    pub chart_id: i64,
    /// Validated on read, so no row loaded here can name an unknown track.
    pub track: TrackId,
    /// Uploaded and imported rows must have a resolved vehicle (enforced by PostgreSQL).
    pub vehicle: VehicleId,
    pub raw_vehicle_name: String,
    pub mod_version: Option<i16>,
    pub lap_time_ms: Milliseconds,
    pub split_1_ms: Milliseconds,
    pub split_2_ms: Milliseconds,
    pub split_3_ms: Milliseconds,
    pub split_4_ms: Milliseconds,
    pub spr_attachment_id: Option<i64>,
    pub raf_attachment_id: Option<i64>,
    pub source: String,
    pub fingerprint: String,
    pub steering: crate::models::hotlap::SteeringInput,
    /// `None` for replays whose source could not report ABS. Not a player
    /// flag: LFS carries it as its own SPR header byte.
    pub abs_enabled: Option<bool>,
    /// Raw 16-bit player flags for steering, driving aids, and driver side.
    /// `steering` is also stored separately for SQL filters.
    pub player_flags: PlayerFlagsBits,
    pub created_at: TimeDateTimeWithTimeZone,
    pub game_version: GameVersionCode,
    pub state: crate::models::hotlap::HotlapState,
    pub hlvc_result_code: Option<i16>,
    pub error_detail: Option<String>,
    pub attempt_count: i32,
    pub next_attempt_at: Option<TimeDateTimeWithTimeZone>,
    pub started_at: Option<TimeDateTimeWithTimeZone>,
    pub finished_at: Option<TimeDateTimeWithTimeZone>,
    #[sea_orm(
        belongs_to,
        from = "player_id",
        to = "id",
        on_update = "Cascade",
        on_delete = "Restrict"
    )]
    pub player: BelongsTo<lfsplanet_core::models::player::Entity>,
    #[sea_orm(belongs_to, from = "chart_id", to = "id", on_delete = "Cascade")]
    pub chart: BelongsTo<crate::models::chart::Entity>,
    #[sea_orm(
        belongs_to,
        from = "era_id",
        to = "id",
        on_update = "Cascade",
        on_delete = "Restrict"
    )]
    pub era: BelongsTo<crate::models::era::Entity>,
    #[sea_orm(
        belongs_to,
        from = "track",
        to = "id",
        on_update = "Cascade",
        on_delete = "Restrict"
    )]
    pub track_relation: BelongsTo<lfsplanet_core::models::track::Entity>,
    #[sea_orm(
        belongs_to,
        from = "vehicle",
        to = "id",
        on_update = "Cascade",
        on_delete = "Restrict"
    )]
    pub vehicle_relation: BelongsTo<lfsplanet_core::models::vehicle::Entity>,
}

impl Model {
    /// Control settings derived from player flags and game version; excludes ABS.
    pub fn controls(&self) -> ControlConfiguration {
        ControlConfiguration::from_player_flags(self.player_flags.0, &self.game_version.0)
    }
}

impl ActiveModelBehavior for ActiveModel {}

/// The `source` value written by the upload path.
///
/// Imported LFS World laps carry a different source and must stay out of
/// every upload-facing read, so the literal is spelled once.
pub const SOURCE_UPLOAD: &str = "upload";

/// Uploads still occupying one of a player's queue slots.
const OUTSTANDING_STATES: [HotlapState; 1] = [HotlapState::Pending];

/// Database filters for hotlap queries.
///
/// Deliberately implemented per builder rather than blanket over
/// `QueryFilter`: a blanket impl would offer `uploads()` on a player query and
/// silently emit `WHERE hotlap.source = 'upload'` against the wrong table.
pub trait HotlapFilter: QueryFilter + Sized {
    /// Player uploads, excluding imported laps.
    fn uploads(self) -> Self {
        self.filter(Column::Source.eq(SOURCE_UPLOAD))
    }

    /// Published laps.
    ///
    /// Only reaches query-builder reads. The raw-SQL reads in `activity`,
    /// `leaderboard` and ranking aggregation spell this predicate themselves, and
    /// all public reads also require current combination membership.
    fn valid(self) -> Self {
        self.filter(Column::State.eq(HotlapState::Valid))
    }

    fn owned_by(self, player_id: i64) -> Self {
        self.filter(Column::PlayerId.eq(player_id))
    }

    fn in_era(self, era_id: i64) -> Self {
        self.filter(Column::EraId.eq(era_id))
    }

    fn in_state(self, state: HotlapState) -> Self {
        self.filter(Column::State.eq(state))
    }

    /// Uploads that have not yet reached a terminal state.
    fn outstanding(self) -> Self {
        self.filter(Column::State.is_in(OUTSTANDING_STATES))
    }
}

impl HotlapFilter for Select<Entity> {}
impl HotlapFilter for UpdateMany<Entity> {}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum HotlapListColumn {
    #[default]
    #[serde(alias = "set")]
    Submitted,
    Driver,
    Rank,
    LapTime,
}
