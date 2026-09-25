//! SPR header diagnostic command.

use std::path::Path;

use anyhow::Context;
use lfsplanet_spr::SprHeader;

/// Parses and prints the fixed metadata header of a local SPR file.
pub(super) fn run(replay: &Path) -> anyhow::Result<()> {
    let header = SprHeader::from_path(replay)
        .with_context(|| format!("failed to parse SPR header from {}", replay.display()))?;
    println!("{header:#?}");
    Ok(())
}
