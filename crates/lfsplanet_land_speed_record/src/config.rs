use clap::Parser;
use insim::core::track::Track;
use lfsplanet_game::{InSimArgs, LayoutArgs, LayoutSpec, VehicleRule};

use crate::LandSpeedRecordLeaderboard;

#[derive(Parser, Debug)]
pub struct LandSpeedRecordArgs {
    #[command(flatten)]
    pub insim: InSimArgs,

    /// LFS track code (e.g. BL1, AS1, SO1).
    #[arg(long, default_value = "FE1X")]
    pub track: Track,

    #[command(flatten)]
    pub layout: LayoutArgs,
}

/// Full runtime configuration for the land_speed_record game.
pub struct LandSpeedRecordRunConfig {
    pub insim: InSimArgs,
    pub config: LandSpeedRecordConfig,
    pub leaderboard: LandSpeedRecordLeaderboard,
}

#[derive(Clone, Debug)]
pub struct LandSpeedRecordConfig {
    pub track: Track,
    pub layout: Option<LayoutSpec>,
    pub vehicle_rules: Vec<VehicleRule>,
}
