//! Command-line parsing and operator-command dispatch.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

mod badge;
mod config;
#[cfg(debug_assertions)]
mod demo;
mod era;
mod hotlap;
mod lfs;
mod maintenance;
mod migrate;
mod openapi;
mod player;
mod seed;
mod storage;
mod track_location;
mod worker;

/// lfspla.net command-line arguments.
#[derive(Debug, Parser)]
#[command(about = "lfspla.net backend")]
pub struct Args {
    /// Path to the YAML configuration file.
    #[arg(
        short = 'c',
        long = "config",
        value_name = "PATH",
        default_value = "planet.yaml",
        global = true
    )]
    pub config: PathBuf,
    /// Process to run.
    #[command(subcommand)]
    pub command: Command,
}

/// Backend process to run.
#[derive(Clone, Debug, Eq, PartialEq, Subcommand)]
pub enum Command {
    /// Apply tagged catalogue YAML definitions.
    Seed(seed::SeedArgs),
    /// Grant, list, and revoke manual catalogue badges.
    Badge {
        #[command(subcommand)]
        action: badge::BadgeCommand,
    },
    /// Generate synthetic players and hotlaps for a development database.
    #[cfg(debug_assertions)]
    Demo(demo::DemoArgs),
    /// Print a basic YAML configuration to standard output.
    GenerateConfig(config::GenerateConfigArgs),
    /// Apply pending database schema migrations and exit.
    Migrate,
    /// Print the OpenAPI document for the public API to standard output.
    Openapi,
    /// Run the web application.
    Web,
    /// Run background record processors.
    Worker,
    /// Validate, import, and reclassify stored hotlaps.
    Hotlap {
        #[command(subcommand)]
        action: hotlap::HotlapCommand,
    },
    /// Add and maintain named LFS installations.
    Lfs {
        #[command(subcommand)]
        action: lfs::LfsCommand,
    },
    /// Manage database-backed era policy.
    Era {
        #[command(subcommand)]
        action: era::EraCommand,
    },
    /// List track locations and delete empty locations.
    TrackLocation {
        #[command(subcommand)]
        action: track_location::TrackLocationCommand,
    },
    /// Inspect and change player account restrictions.
    Player {
        #[command(subcommand)]
        action: player::PlayerCommand,
    },
    /// Run one-shot maintenance tasks.
    Maintenance {
        #[command(subcommand)]
        action: maintenance::MaintenanceCommand,
    },
}

/// Dispatches one parsed command.
pub(crate) async fn run(args: &Args) -> anyhow::Result<()> {
    match &args.command {
        Command::Seed(options) => seed::run(args, options).await,
        Command::Badge { action } => badge::run(args, action).await,
        #[cfg(debug_assertions)]
        Command::Demo(options) => demo::run(args, options).await,
        Command::GenerateConfig(options) => config::run(options),
        Command::Migrate => migrate::run(args).await,
        Command::Openapi => openapi::run(),
        Command::Worker => worker::run(args).await,
        Command::Web => crate::api::run(args).await,
        Command::Hotlap { action } => hotlap::run(args, action).await,
        Command::Lfs { action } => lfs::run(args, action).await,
        Command::Era { action } => era::run(args, action).await,
        Command::TrackLocation { action } => track_location::run(args, action).await,
        Command::Player { action } => player::run(args, action).await,
        Command::Maintenance { action } => maintenance::run(args, action).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalogue_commands_require_the_current_names() {
        assert!(Args::try_parse_from(["lfsplanet", "maintenance", "sync-mods"]).is_ok());
        assert!(Args::try_parse_from(["lfsplanet", "seed", "era.yaml"]).is_ok());
        assert!(Args::try_parse_from(["lfsplanet", "maintenance", "catalogue-sync"]).is_err());
        assert!(Args::try_parse_from(["lfsplanet", "maintenance", "mods-sync"]).is_err());
        assert!(Args::try_parse_from(["lfsplanet", "era", "apply", "era.yaml"]).is_err());
    }

    #[test]
    fn event_sync_modes_and_positive_league_are_validated() {
        assert!(
            Args::try_parse_from(["lfsplanet", "maintenance", "sync-events", "--dry-run"]).is_ok()
        );
        assert!(
            Args::try_parse_from([
                "lfsplanet",
                "maintenance",
                "sync-events",
                "--dry-run",
                "--league",
                "432"
            ])
            .is_ok()
        );
        assert!(Args::try_parse_from(["lfsplanet", "maintenance", "sync-events"]).is_ok());
        assert!(
            Args::try_parse_from(["lfsplanet", "maintenance", "sync-events", "--source-only"])
                .is_ok()
        );
        assert!(
            Args::try_parse_from([
                "lfsplanet",
                "maintenance",
                "sync-events",
                "--source-only",
                "--dry-run"
            ])
            .is_err()
        );
        assert!(
            Args::try_parse_from([
                "lfsplanet",
                "maintenance",
                "sync-events",
                "--dry-run",
                "--league",
                "0"
            ])
            .is_err()
        );
    }
}
