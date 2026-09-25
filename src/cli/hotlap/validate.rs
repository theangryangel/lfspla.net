//! One-shot, operator-visible HLVC diagnostics.

use std::path::Path;

use anyhow::Context;

use crate::{cli::Args, hlvc::diagnose, settings::Settings};

/// Validates one local replay without storing or publishing anything.
pub(super) async fn run(
    args: &Args,
    installation_id: &str,
    replay_path: &Path,
) -> anyhow::Result<()> {
    let settings = Settings::load(&args.config)?;
    let replay = tokio::fs::read(replay_path)
        .await
        .with_context(|| format!("failed to read replay {}", replay_path.display()))?;
    let diagnostic = diagnose(
        &settings.lfs_runtime,
        &settings.worker.hlvc,
        installation_id,
        &replay,
    )
    .await?;

    println!("Bubblewrap status: {}", diagnostic.output.status);
    println!(
        "--- Bubblewrap stdout ---\n{}",
        String::from_utf8_lossy(&diagnostic.output.stdout)
    );
    eprintln!(
        "--- Wine and Bubblewrap stderr ---\n{}",
        String::from_utf8_lossy(&diagnostic.output.stderr)
    );

    let result = diagnostic
        .result()
        .context("could not extract a complete HLVC status from Bubblewrap output")?;
    println!("Bubblewrap exit code: {}", result.bubblewrap_exit_code);
    println!("Wine exit code: {}", result.wine_exit_code);
    println!(
        "LFS HLVC result: {} ({})",
        result.lfs.code(),
        result.lfs.description()
    );
    Ok(())
}
