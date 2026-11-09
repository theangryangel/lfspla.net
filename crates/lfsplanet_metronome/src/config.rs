use std::time::Duration;

use clap::Parser;
use insim::core::track::Track;
use lfsplanet_game::{InSimArgs, LayoutArgs, LayoutSpec, VehicleRule};

use crate::MetronomeLeaderboard;

#[derive(Parser, Debug)]
pub struct MetronomeArgs {
    #[command(flatten)]
    pub insim: InSimArgs,

    /// Target duration in milliseconds that players should aim to match.
    #[arg(long, default_value = "30000")]
    pub target_ms: u64,

    /// LFS track code (e.g. BL1, AS1, SO1).
    #[arg(long, default_value = "FE1X")]
    pub track: Track,

    #[command(flatten)]
    pub layout: LayoutArgs,
}

/// Full runtime configuration for the metronome game.
pub struct MetronomeRunConfig {
    pub insim: InSimArgs,
    pub config: MetronomeConfig,
    pub leaderboard: MetronomeLeaderboard,
}

#[derive(Clone, Debug)]
pub struct MetronomeConfig {
    pub target: Duration,
    pub track: Track,
    pub layout: Option<LayoutSpec>,
    pub vehicle_rules: Vec<VehicleRule>,
}
