//! Event-specific source and persisted mini-game settings.
use sea_orm::{DeriveActiveEnum, EnumIter};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    EnumIter,
    DeriveActiveEnum,
    Serialize,
    Deserialize,
    ToSchema,
)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    #[default]
    #[sea_orm(string_value = "local")]
    Local,
    #[sea_orm(string_value = "lfs_net")]
    LfsNet,
    #[sea_orm(string_value = "bomb")]
    Bomb,
    #[sea_orm(string_value = "metronome")]
    Metronome,
    #[sea_orm(string_value = "shortcut")]
    Shortcut,
    #[sea_orm(string_value = "land_speed_record")]
    LandSpeedRecord,
}

/// The source selects both the runtime and result classification.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum GameSource {
    Bomb {
        checkpoint_timeout_secs: u64,
        checkpoint_penalty_ms: u64,
        collision_max_penalty_ms: u64,
    },
    Metronome {
        target_ms: u64,
    },
    Shortcut,
    LandSpeedRecord,
}
impl GameSource {
    pub(crate) fn kind(&self) -> EventKind {
        match self {
            Self::Bomb { .. } => EventKind::Bomb,
            Self::Metronome { .. } => EventKind::Metronome,
            Self::Shortcut => EventKind::Shortcut,
            Self::LandSpeedRecord => EventKind::LandSpeedRecord,
        }
    }
    pub(crate) fn validate(&self) -> anyhow::Result<()> {
        match self {
            Self::Bomb {
                checkpoint_timeout_secs,
                checkpoint_penalty_ms,
                collision_max_penalty_ms,
            } => {
                anyhow::ensure!(
                    *checkpoint_timeout_secs > 0 && *checkpoint_timeout_secs <= 86400,
                    "checkpoint timeout must be between 1 and 86400 seconds"
                );
                anyhow::ensure!(
                    *checkpoint_penalty_ms <= 86400000 && *collision_max_penalty_ms <= 86400000,
                    "penalties must not exceed a day"
                );
            }
            Self::Metronome { target_ms } => anyhow::ensure!(
                *target_ms > 0 && *target_ms <= 86400000,
                "target must be between 1ms and a day"
            ),
            Self::Shortcut | Self::LandSpeedRecord => {}
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct VehicleRule {
    pub vehicle: String,
    #[serde(default)]
    pub added_mass_kg: u16,
    #[serde(default)]
    pub intake_restriction: u8,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct GameConfig {
    pub source: GameSource,
    pub track: String,
    #[serde(default)]
    pub vehicles: Vec<VehicleRule>,
}
impl GameConfig {
    pub(crate) fn validate(&self) -> anyhow::Result<()> {
        self.source.validate()?;
        self.track.parse::<insim::core::track::Track>()?;
        let mut seen = std::collections::HashSet::new();
        for rule in &self.vehicles {
            let vehicle = rule.vehicle.parse::<insim::core::vehicle::Vehicle>()?;
            anyhow::ensure!(
                vehicle != insim::core::vehicle::Vehicle::Unknown,
                "unknown vehicle {}",
                rule.vehicle
            );
            anyhow::ensure!(
                seen.insert(rule.vehicle.to_ascii_uppercase()),
                "duplicate vehicle"
            );
            anyhow::ensure!(
                rule.added_mass_kg <= 200 && rule.intake_restriction <= 50,
                "invalid vehicle restrictions"
            );
        }
        Ok(())
    }
}
impl super::Model {
    pub(crate) fn game(&self) -> anyhow::Result<Option<GameConfig>> {
        let config = self
            .game_config
            .clone()
            .map(serde_json::from_value::<GameConfig>)
            .transpose()?;
        if let Some(config) = &config {
            anyhow::ensure!(
                config.source.kind() == self.source,
                "event source/configuration mismatch"
            );
            config.validate()?;
        } else {
            anyhow::ensure!(
                matches!(self.source, EventKind::Local | EventKind::LfsNet),
                "game event missing configuration"
            );
        }
        Ok(config)
    }
}
