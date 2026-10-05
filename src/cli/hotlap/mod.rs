//! Operator CLI and background process for stored hotlaps.

use std::path::PathBuf;

use clap::Subcommand;

/// Stored hotlap operation.
#[derive(Clone, Debug, Eq, PartialEq, Subcommand)]
pub(crate) enum HotlapCommand {
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
        installation_id: String,
        /// Local SPR file to validate.
        #[arg(value_name = "SPR")]
        replay: PathBuf,
    },
    /// Import hotlaps from LFSWorld v1 CSV exports.
    ImportLfsworld {
        /// Paths to LFSWorld v1 hotlap CSV exports; shell globs may be used.
        #[arg(value_name = "HOTLAPS_CSV", required = true)]
        hotlaps_csv: Vec<PathBuf>,
    },
    /// Reclassify hotlaps and rebuild personal bests and chart positions.
    FixEras,
    /// Rebuild player badges for selected eras.
    Rebadge {
        #[command(flatten)]
        scope: RebadgeScope,
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
    /// Rebuild one era; repeat this option to select multiple eras.
    #[arg(long = "era", value_name = "ID")]
    pub(crate) eras: Vec<String>,
}

mod fix_eras;
mod import_lfsworld;
mod inspect;
mod rebadge;
mod validate;

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
        HotlapCommand::Validate {
            installation_id,
            replay,
        } => validate::run(args, installation_id, replay).await,
    }
}
