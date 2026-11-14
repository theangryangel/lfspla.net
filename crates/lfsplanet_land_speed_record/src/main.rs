#![allow(missing_docs, missing_debug_implementations)]
//! Standalone LandSpeedRecord runner.

use clap::Parser;
use insim as _;
use kitcar as _;
use lfsplanet_game as _;
use lfsplanet_land_speed_record::LandSpeedRecordArgs;
use parking_lot as _;
use tokio_util as _;
use tracing as _;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::builder()
                .with_default_directive(tracing_subscriber::filter::LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .init();

    let args = LandSpeedRecordArgs::parse();
    lfsplanet_land_speed_record::run_land_speed_record(args)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))
}
