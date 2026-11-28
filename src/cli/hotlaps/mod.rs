//! Operator commands owned by the hotlaps crate: stored hotlaps, eras and LFS
//! installations.

use lfsplanet_hotlaps::installation_id::InstallationId;
use std::path::PathBuf;

use clap::Subcommand;

/// Hotlaps operation.
#[derive(Clone, Debug, Eq, PartialEq, Subcommand)]
pub(crate) enum HotlapsCommand {
    /// Parse an SPR header and print its metadata for diagnostics.
    Inspect {
        /// Local SPR file to inspect.
        #[arg(value_name = "SPR")]
        replay: PathBuf,
    },
    /// Run one local SPR through HLVC and print its diagnostic output.
    Validate {
        /// Named LFS installation to use.
        #[arg(value_name = "INSTALLATION_ID")]
        installation_id: InstallationId,
        /// Local SPR file to validate.
        #[arg(value_name = "SPR")]
        replay: PathBuf,
    },
    /// Import hotlaps from LFSWorld v1 CSV exports.
    ImportLfsworld {
        /// Paths to LFSWorld v1 hotlap CSV exports; shell globs may be used.
        #[arg(value_name = "HOTLAPS_CSV", required = true)]
        hotlaps_csv: Vec<PathBuf>,
        /// Recursively search this directory for matching SPR attachments.
        #[arg(long, value_name = "DIRECTORY")]
        spr_dir: Option<PathBuf>,
    },
    /// Check stored SPRs in an explicitly selected scope.
    VerifySprs {
        /// Delete hotlaps whose SPRs fail parsing; storage read failures are retained.
        #[arg(long, global = true)]
        delete_invalid: bool,
        #[command(subcommand)]
        scope: verify_sprs::Scope,
    },
    /// Delete a stored hotlap and restore affected personal bests and rankings.
    Delete {
        #[arg(value_name = "HOTLAP_ID", value_parser = clap::value_parser!(i64).range(1..))]
        id: i64,
    },
    /// Reclassify hotlaps and rebuild personal bests and chart positions.
    FixEras,
    /// Rebuild player badges for selected eras.
    Rebadge {
        #[command(flatten)]
        scope: RebadgeScope,
    },
    /// Manage database-backed era policy.
    Era {
        #[command(subcommand)]
        action: era::EraCommand,
    },
    /// Add and maintain named LFS installations.
    Installation {
        #[command(subcommand)]
        action: installation::InstallationCommand,
    },
}

/// Mutually exclusive era selection for badge rebuilding.
///
/// Selects what a manual repair run covers.
#[derive(Clone, Debug, Eq, PartialEq, clap::Args)]
#[group(required = true, multiple = false)]
pub(crate) struct RebadgeScope {
    /// Rebuild every configured era.
    #[arg(long)]
    pub(crate) all: bool,
    /// Rebuild every era currently open for hotlap uploads.
    #[arg(long)]
    pub(crate) open: bool,
    /// Retry eras whose queued badge refresh has not completed.
    #[arg(long)]
    pub(crate) pending: bool,
    /// Rebuild one era; repeat this option to select multiple eras.
    #[arg(long = "era", value_name = "ID")]
    pub(crate) eras: Vec<lfsplanet_hotlaps::era_slug::EraSlug>,
}

mod delete;
mod era;
mod fix_eras;
mod import_lfsworld;
mod inspect;
mod installation;
mod rebadge;
mod validate;
mod verify_sprs;

use crate::cli::Args;

/// Runs one hotlaps command.
pub(crate) async fn run(args: &Args, action: &HotlapsCommand) -> anyhow::Result<()> {
    match action {
        HotlapsCommand::VerifySprs {
            scope,
            delete_invalid,
        } => verify_sprs::run(args, scope, *delete_invalid).await,
        HotlapsCommand::Delete { id } => delete::run(args, *id).await,
        HotlapsCommand::Inspect { replay } => inspect::run(replay),
        HotlapsCommand::ImportLfsworld {
            hotlaps_csv,
            spr_dir,
        } => import_lfsworld::run(args, hotlaps_csv, spr_dir.as_deref()).await,
        HotlapsCommand::FixEras => fix_eras::run(args).await,
        HotlapsCommand::Rebadge { scope } => rebadge::run(args, scope).await,
        HotlapsCommand::Validate {
            installation_id,
            replay,
        } => validate::run(args, installation_id, replay).await,
        HotlapsCommand::Era { action } => era::run(args, action).await,
        HotlapsCommand::Installation { action } => installation::run(args, action).await,
    }
}
