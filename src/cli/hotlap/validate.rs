//! One-shot, operator-visible HLVC diagnostics.

use std::path::Path;

use anyhow::Context;

use crate::{cli::Args, services::manage_lfs::resolve_installation, settings::Settings};

/// Validates one local replay without storing or publishing anything.
pub(super) async fn run(
    args: &Args,
    installation_id: &str,
    replay_path: &Path,
) -> anyhow::Result<()> {
    lfsplanet_lfs::ensure_supported()?;
    let settings = Settings::load(&args.config)?;
    let replay = tokio::fs::read(replay_path)
        .await
        .with_context(|| format!("failed to read replay {}", replay_path.display()))?;
    let installation =
        resolve_installation(settings.lfs.installation_root.path(), installation_id)?;
    let diagnostic = lfsplanet_lfs::validate(
        &settings.lfs.runtime,
        &installation,
        &replay,
        settings.worker.hlvc.timeout.duration(),
    )
    .await?;

    println!("Runner status: {}", diagnostic.output.status);
    println!(
        "--- Runner stdout ---\n{}",
        String::from_utf8_lossy(&diagnostic.output.stdout)
    );
    eprintln!(
        "--- Runtime stderr ---\n{}",
        String::from_utf8_lossy(&diagnostic.output.stderr)
    );

    let result = diagnostic
        .result()
        .context("could not obtain a complete HLVC result")?;
    println!("Runner exit code: {}", result.process_exit_code);
    println!("Runtime exit code: {}", result.runtime_exit_code);
    println!(
        "LFS HLVC result: {} ({})",
        result.lfs.code(),
        result.lfs.description()
    );
    Ok(())
}
