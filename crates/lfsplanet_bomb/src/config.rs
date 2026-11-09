use std::time::Duration;

use clap::Parser;
use insim::core::track::Track;
use lfsplanet_game::{InSimArgs, LayoutArgs, LayoutSpec, VehicleRule};

use crate::BombLeaderboard;

pub(super) const TICK_PERIOD: Duration = Duration::from_millis(500);
pub(super) const COLLISION_THRESHOLD_MPS: f32 = 30.0;
pub(super) const PENALTY_CLEAR_DELAY: Duration = Duration::from_secs(15);

#[derive(Parser, Debug)]
pub struct BombArgs {
    #[command(flatten)]
    pub insim: InSimArgs,

    /// LFS track code (e.g. BL1, AS1, SO1).
    #[arg(long, default_value = "FE1X")]
    pub track: Track,

    #[command(flatten)]
    pub layout: LayoutArgs,
}

/// Full runtime configuration for the bomb game.
pub struct BombRunConfig {
    pub insim: InSimArgs,
    pub config: BombConfig,
    pub leaderboard: BombLeaderboard,
}

#[derive(Clone, Debug)]
pub struct BombConfig {
    pub checkpoint_timeout: Duration,
    pub checkpoint_penalty: Duration,
    pub collision_max_penalty: Duration,
    pub track: Track,
    pub layout: Option<LayoutSpec>,
    pub vehicle_rules: Vec<VehicleRule>,
}

impl Default for BombConfig {
    fn default() -> Self {
        Self {
            checkpoint_timeout: Duration::from_secs(30),
            checkpoint_penalty: Duration::from_millis(250),
            collision_max_penalty: Duration::from_millis(500),
            track: Track::default(),
            layout: None,
            vehicle_rules: Vec::new(),
        }
    }
}
