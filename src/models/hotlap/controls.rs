//! Hotlap state and replay control semantics.

use insim_core::game_version::GameVersion;
use lfsplanet_spr::PlayerFlags;
use sea_orm::{DeriveActiveEnum, EnumIter};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Steering input derived from the SPR player flags.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize, ToSchema,
)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "snake_case")]
pub enum SteeringInput {
    #[sea_orm(string_value = "wheel")]
    Wheel,
    #[sea_orm(string_value = "mouse")]
    Mouse,
    #[sea_orm(string_value = "keyboard")]
    Keyboard,
    #[sea_orm(string_value = "keyboard_stabilised")]
    KeyboardStabilised,
}

impl SteeringInput {
    /// Derives the steering category using the documented flag precedence.
    pub fn from_player_flags(flags: PlayerFlags) -> Self {
        if flags.contains(PlayerFlags::MOUSE) {
            Self::Mouse
        } else if flags.contains(PlayerFlags::KB_STABILISED) {
            Self::KeyboardStabilised
        } else if flags.contains(PlayerFlags::KB_NO_HELP) {
            Self::Keyboard
        } else {
            Self::Wheel
        }
    }
}

/// Side of the vehicle occupied by the driver.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize, ToSchema,
)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "snake_case")]
pub enum DriverSide {
    #[sea_orm(string_value = "left")]
    Left,
    #[sea_orm(string_value = "right")]
    Right,
}

/// Processing and publication state for a hotlap.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize, ToSchema,
)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "snake_case")]
pub enum HotlapState {
    #[sea_orm(string_value = "pending")]
    Pending,
    #[sea_orm(string_value = "valid")]
    Valid,
    #[sea_orm(string_value = "invalid")]
    Invalid,
}

/// First LFS version whose player flags carry `SHIFTER`.
///
/// The bit is set in 0 of the 43,437 archived 0.5P-0.5Q laps, while flags as
/// rare as `KB_NO_HELP` still register 115 of them there, so a cleared bit
/// before this version records that LFS could not express the setting.
const SHIFTER_INTRODUCED: GameVersion = GameVersion {
    major: 0.5,
    minor: 'R',
    patch: None,
};

/// First LFS version that reports ABS.
///
/// Both the SPR header byte and LFSWorld's `abs` column are zero for every
/// archived lap before this version, then 137 of the first 799 at 0.5Z25.
const ABS_INTRODUCED: GameVersion = GameVersion {
    major: 0.5,
    minor: 'Z',
    patch: Some(25),
};

/// Resolves one version-gated setting.
///
/// An enabled setting is always a real observation: LFS cannot report a feature
/// it does not have. A disabled one is only an observation once the version
/// could express it, and is otherwise unknown.
fn observed_since(
    enabled: bool,
    game_version: &GameVersion,
    introduced: &GameVersion,
) -> Option<bool> {
    (enabled || game_version >= introduced).then_some(enabled)
}

/// Resolves the replay's ABS setting against the version that reported it.
pub fn resolve_abs(enabled: bool, game_version: &GameVersion) -> Option<bool> {
    observed_since(enabled, game_version, &ABS_INTRODUCED)
}

/// Stable control semantics interpreted from one replay's player flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControlConfiguration {
    pub steering: SteeringInput,
    pub brake_help_enabled: bool,
    pub automatic_gears: bool,
    /// `None` for replays predating [`SHIFTER_INTRODUCED`].
    pub manual_shifter: Option<bool>,
    pub axis_clutch: bool,
    pub automatic_clutch: bool,
    pub driver_side: DriverSide,
}

impl ControlConfiguration {
    /// Interprets player flags in the context of the version that wrote them.
    ///
    /// Only `SHIFTER` is version-gated. Every other bit read here is set
    /// somewhere in all three LFSWorld export generations with a stable
    /// distribution, so a cleared bit is a genuine observation of "off".
    ///
    /// The flags this deliberately ignores are not control settings or are not
    /// evidenced: `CUSTOM_VIEW` is a camera setting rather than an input, and
    /// `FLEXIBLE_STEER` and `INPITS` are set in none of the 164,721 archived
    /// laps, so any version gate for them would be a guess.
    pub fn from_player_flags(flags: PlayerFlags, game_version: &GameVersion) -> Self {
        Self {
            steering: SteeringInput::from_player_flags(flags),
            brake_help_enabled: flags.contains(PlayerFlags::HELP_B),
            automatic_gears: flags.contains(PlayerFlags::AUTOGEARS),
            manual_shifter: observed_since(
                flags.contains(PlayerFlags::SHIFTER),
                game_version,
                &SHIFTER_INTRODUCED,
            ),
            axis_clutch: flags.contains(PlayerFlags::AXIS_CLUTCH),
            automatic_clutch: flags.contains(PlayerFlags::AUTOCLUTCH),
            driver_side: if flags.contains(PlayerFlags::LEFTSIDE) {
                DriverSide::Left
            } else {
                DriverSide::Right
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use lfsplanet_spr::PlayerFlags;

    use super::*;

    #[test]
    fn derives_steering_from_player_flags() {
        assert_eq!(
            SteeringInput::from_player_flags(PlayerFlags::MOUSE),
            SteeringInput::Mouse
        );
        assert_eq!(
            SteeringInput::from_player_flags(PlayerFlags::KB_NO_HELP),
            SteeringInput::Keyboard
        );
        assert_eq!(
            SteeringInput::from_player_flags(PlayerFlags::KB_STABILISED),
            SteeringInput::KeyboardStabilised
        );
        assert_eq!(
            SteeringInput::from_player_flags(PlayerFlags::empty()),
            SteeringInput::Wheel
        );
    }

    fn version(text: &str) -> GameVersion {
        GameVersion::from_str(text).expect("test version should parse")
    }

    #[test]
    fn normalizes_control_configuration_from_player_flags() {
        let controls = ControlConfiguration::from_player_flags(
            PlayerFlags::LEFTSIDE
                | PlayerFlags::AUTOGEARS
                | PlayerFlags::SHIFTER
                | PlayerFlags::HELP_B
                | PlayerFlags::AXIS_CLUTCH
                | PlayerFlags::AUTOCLUTCH
                | PlayerFlags::MOUSE,
            &version("0.7G"),
        );

        assert_eq!(controls.steering, SteeringInput::Mouse);
        assert!(controls.brake_help_enabled);
        assert!(controls.automatic_gears);
        assert_eq!(controls.manual_shifter, Some(true));
        assert!(controls.axis_clutch);
        assert!(controls.automatic_clutch);
        assert_eq!(controls.driver_side, DriverSide::Left);
    }

    #[test]
    fn reports_the_shifter_as_unknown_only_when_the_version_predates_the_flag() {
        // A cleared bit is an absence of information before 0.5R...
        assert_eq!(
            ControlConfiguration::from_player_flags(PlayerFlags::empty(), &version("0.5P"))
                .manual_shifter,
            None
        );
        // ...but a real observation once LFS could express it.
        assert_eq!(
            ControlConfiguration::from_player_flags(PlayerFlags::empty(), &version("0.5R"))
                .manual_shifter,
            Some(false)
        );
        // A set bit is always an observation: LFS cannot report what it lacks.
        assert_eq!(
            ControlConfiguration::from_player_flags(PlayerFlags::SHIFTER, &version("0.5P"))
                .manual_shifter,
            Some(true)
        );
    }

    #[test]
    fn reports_abs_as_unknown_only_when_the_version_predates_it() {
        assert_eq!(resolve_abs(false, &version("0.5Y14")), None);
        // Patch numbers compare numerically: 0.5Z4 precedes 0.5Z25.
        assert_eq!(resolve_abs(false, &version("0.5Z4")), None);
        assert_eq!(resolve_abs(false, &version("0.5Z25")), Some(false));
        assert_eq!(resolve_abs(false, &version("0.6B")), Some(false));
        assert_eq!(resolve_abs(true, &version("0.5P")), Some(true));
    }
}
