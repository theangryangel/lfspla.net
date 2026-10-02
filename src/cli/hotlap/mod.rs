//! Operator CLI and background process for stored hotlaps.

mod args;
mod fix_eras;
mod import_lfsworld;
mod inspect;
mod rebadge;
#[cfg(target_os = "linux")]
mod validate;

pub(crate) use args::{HotlapCommand, RebadgeScope};

use crate::cli::Args;

/// Runs one hotlap command.
pub(crate) async fn run(args: &Args, action: &HotlapCommand) -> anyhow::Result<()> {
    match action {
        HotlapCommand::Inspect { replay } => inspect::run(replay),
        HotlapCommand::ImportLfsworld { hotlaps_csv } => {
            import_lfsworld::run(args, hotlaps_csv).await
        }
        HotlapCommand::FixEras => fix_eras::run(args).await,
        HotlapCommand::Rebadge { scope } => rebadge::run(args, scope).await,
        #[cfg(target_os = "linux")]
        HotlapCommand::Validate {
            installation_id,
            replay,
        } => validate::run(args, installation_id, replay).await,
        #[cfg(not(target_os = "linux"))]
        HotlapCommand::Validate { .. } => {
            tracing::error!(
                "hotlap validation is unsupported on this platform; it requires Linux and Bubblewrap"
            );
            anyhow::bail!(
                "hotlap validation is unsupported on this platform; it requires Linux and Bubblewrap"
            )
        }
    }
}
