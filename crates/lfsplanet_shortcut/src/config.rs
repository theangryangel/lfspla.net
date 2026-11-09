use clap::Parser;
use insim::core::track::Track;
use lfsplanet_game::{InSimArgs, LayoutArgs, LayoutSpec, VehicleRule};

use crate::ShortcutLeaderboard;

#[derive(Parser, Debug)]
pub struct ShortcutArgs {
    #[command(flatten)]
    pub insim: InSimArgs,

    /// LFS track code (e.g. BL1, AS1, SO1).
    #[arg(long, default_value = "FE1X")]
    pub track: Track,

    #[command(flatten)]
    pub layout: LayoutArgs,
}

/// Full runtime configuration for the shortcut game.
pub struct ShortcutRunConfig {
    pub insim: InSimArgs,
    pub config: ShortcutConfig,
    pub leaderboard: ShortcutLeaderboard,
}

#[derive(Clone, Debug)]
pub struct ShortcutConfig {
    pub track: Track,
    pub layout: Option<LayoutSpec>,
    pub vehicle_rules: Vec<VehicleRule>,
}
