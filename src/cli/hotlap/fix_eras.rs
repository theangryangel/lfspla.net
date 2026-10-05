//! Operator command for repairing stored hotlap era classifications.

use crate::{cli::Args, db, services::repair_eras, settings::Settings};

pub(super) async fn run(args: &Args) -> anyhow::Result<()> {
    let settings = Settings::load(&args.config)?;
    let database = db::connect(&settings.database, 2).await?;
    let summary = repair_eras::repair(&database).await?;
    tracing::info!(
        game_versions = summary.game_versions,
        hotlaps_updated = summary.hotlaps_updated,
        "stored era classifications and chart positions are current"
    );
    Ok(())
}
