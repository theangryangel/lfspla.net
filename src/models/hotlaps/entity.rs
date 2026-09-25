//! SeaORM entity for the `hotlap` table.

use std::collections::HashSet;

use object_store::ObjectMeta;
use sea_orm::entity::prelude::*;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QuerySelect};

use crate::models::hotlaps::ControlConfiguration;
use crate::storage::Storage;

use crate::models::{
    player_flags::PlayerFlagsBits, track::TrackId, vehicle::VehicleId, version::GameVersionCode,
};

/// A recorded lap throughout upload/import and validation.
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "hotlap")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub player_id: i64,
    pub era_id: i64,
    /// Validated on read, so no row loaded here can name an unknown track.
    pub track: TrackId,
    /// Uploaded and imported rows must have a resolved vehicle (enforced by PostgreSQL).
    pub vehicle: Option<VehicleId>,
    pub raw_vehicle_name: String,
    pub mod_version: Option<i16>,
    pub lap_time_ms: i64,
    pub split_1_ms: i64,
    pub split_2_ms: i64,
    pub split_3_ms: i64,
    pub split_4_ms: i64,
    pub original_filename: Option<String>,
    pub spr_object_key: Option<String>,
    pub source: String,
    pub fingerprint: String,
    pub steering: crate::models::hotlaps::SteeringInput,
    /// `None` for replays whose source could not report ABS. Not a player
    /// flag: LFS carries it as its own SPR header byte.
    pub abs_enabled: Option<bool>,
    /// Raw 16-bit player flags for steering, driving aids, and driver side.
    /// `steering` is also stored separately for SQL filters.
    pub player_flags: PlayerFlagsBits,
    pub created_at: TimeDateTimeWithTimeZone,
    pub game_version: GameVersionCode,
    pub state: crate::models::hotlaps::HotlapState,
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
    pub player: BelongsTo<crate::models::players::Entity>,
    #[sea_orm(
        belongs_to,
        from = "era_id",
        to = "id",
        on_update = "Cascade",
        on_delete = "Restrict"
    )]
    pub era: BelongsTo<crate::models::eras::Entity>,
    #[sea_orm(
        belongs_to,
        from = "track",
        to = "id",
        on_update = "Cascade",
        on_delete = "Restrict"
    )]
    pub track_relation: BelongsTo<crate::models::tracks::Entity>,
    #[sea_orm(
        belongs_to,
        from = "vehicle",
        to = "id",
        on_update = "Cascade",
        on_delete = "Restrict"
    )]
    pub vehicle_relation: BelongsTo<Option<crate::models::vehicles::Entity>>,
}

impl Model {
    /// Control settings derived from player flags and game version; excludes ABS.
    pub fn controls(&self) -> ControlConfiguration {
        ControlConfiguration::from_player_flags(self.player_flags.0, &self.game_version.0)
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Storage for Entity {
    const PREFIX: &'static str = "spr";

    async fn should_gc(
        database: &DatabaseConnection,
        objects: &[ObjectMeta],
    ) -> Result<Vec<ObjectMeta>, DbErr> {
        let candidates = objects
            .iter()
            .map(|object| object.location.to_string())
            .collect::<Vec<_>>();
        let retained = Entity::find()
            .select_only()
            .column(Column::SprObjectKey)
            .filter(Column::SprObjectKey.is_in(candidates))
            .into_tuple::<String>()
            .all(database)
            .await?
            .into_iter()
            .collect::<HashSet<_>>();
        Ok(objects
            .iter()
            .filter(|object| !retained.contains(object.location.as_ref()))
            .cloned()
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use insim_core::{game_version::GameVersion, track::Track, vehicle::Vehicle};

    use super::*;
    use crate::models::{
        hotlaps::{DriverSide, HotlapState, SteeringInput},
        player_flags::PlayerFlagsBits,
        track::TrackId,
        vehicle::VehicleId,
        version::GameVersionCode,
    };

    fn version(text: &str) -> GameVersion {
        GameVersion::from_str(text).expect("test version should parse")
    }

    #[test]
    fn derives_every_control_setting_from_the_stored_flags() {
        let model = Model {
            id: 7,
            player_id: 3,
            era_id: 1,
            track: TrackId(Track::So4r),
            vehicle: Some(VehicleId(Vehicle::Rb4)),
            raw_vehicle_name: "RB4".to_owned(),
            mod_version: None,
            lap_time_ms: 118_200,
            split_1_ms: 34_680,
            split_2_ms: 78_230,
            split_3_ms: 118_200,
            split_4_ms: 0,
            original_filename: Some("example_SO4R_RB4_158200.spr".to_owned()),
            spr_object_key: Some("spr/ab/abcdef.spr".to_owned()),
            source: "upload".to_owned(),
            fingerprint: "ab".repeat(32),
            steering: SteeringInput::Mouse,
            abs_enabled: Some(true),
            // LEFTSIDE | <unnamed 1> | <unnamed 2> | AUTOGEARS | AUTOCLUTCH | MOUSE
            player_flags: PlayerFlagsBits(lfsplanet_spr::PlayerFlags::from_bits_retain(
                1 | 2 | 4 | 8 | 512 | 1024,
            )),
            created_at: time::OffsetDateTime::UNIX_EPOCH,
            game_version: GameVersionCode(version("0.7B")),
            state: HotlapState::Valid,
            hlvc_result_code: Some(1),
            error_detail: None,
            attempt_count: 1,
            next_attempt_at: None,
            started_at: Some(time::OffsetDateTime::UNIX_EPOCH),
            finished_at: Some(time::OffsetDateTime::UNIX_EPOCH),
        };

        // Returned controls use the flags, not the separately stored steering value.
        let controls = model.controls();
        assert_eq!(controls.steering, SteeringInput::Mouse);
        assert!(controls.automatic_gears);
        assert!(controls.automatic_clutch);
        assert!(!controls.brake_help_enabled);
        assert!(!controls.axis_clutch);
        assert_eq!(controls.driver_side, DriverSide::Left);
        // 0.7B postdates the shifter flag, so a clear bit is a real "off".
        assert_eq!(controls.manual_shifter, Some(false));
    }
}
