//! Command-line parsing and operator-command dispatch.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

mod badges;
mod config;
#[cfg(debug_assertions)]
mod demo;
mod gc;
mod hotlaps;
mod migrate;
mod openapi;
mod players;
mod race_calendar;
mod seed;
mod storage;
mod track_locations;
mod vehicles;
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
    /// Run the web application.
    Web,
    /// Run background record processors.
    Worker,
    /// Apply pending database schema migrations and exit.
    Migrate,
    /// Apply tagged catalogue YAML definitions.
    Seed(seed::SeedArgs),
    /// Print a basic YAML configuration to standard output.
    GenerateConfig(config::GenerateConfigArgs),
    /// Print the OpenAPI document for the public API to standard output.
    Openapi,
    /// Generate synthetic players and hotlaps for a development database.
    #[cfg(debug_assertions)]
    Demo(demo::DemoArgs),
    /// Inspect and change player account restrictions.
    Players {
        #[command(subcommand)]
        action: players::PlayersCommand,
    },
    /// Grant, list, and revoke manual catalogue badges.
    Badges {
        #[command(subcommand)]
        action: badges::BadgesCommand,
    },
    /// List track locations and delete empty locations.
    TrackLocations {
        #[command(subcommand)]
        action: track_locations::TrackLocationsCommand,
    },
    /// Hotlaps: stored hotlaps, eras and LFS installations.
    Hotlaps {
        #[command(subcommand)]
        action: hotlaps::HotlapsCommand,
    },
    /// Race calendar: LFS leagues, seasons and event advertisements.
    RaceCalendar {
        #[command(subcommand)]
        action: race_calendar::RaceCalendarCommand,
    },
    /// Vehicles, including Vehicle Mod metadata and covers from the LFS API.
    Vehicles {
        #[command(subcommand)]
        action: vehicles::VehiclesCommand,
    },
    /// Delete expired sessions, stale access tokens and unreferenced storage.
    Gc(gc::GcArgs),
}

/// Dispatches one parsed command.
pub(crate) async fn run(args: &Args) -> anyhow::Result<()> {
    match &args.command {
        Command::Seed(options) => seed::run(args, options).await,
        Command::Badges { action } => badges::run(args, action).await,
        #[cfg(debug_assertions)]
        Command::Demo(options) => demo::run(args, options).await,
        Command::GenerateConfig(options) => config::run(options),
        Command::Migrate => migrate::run(args).await,
        Command::Openapi => openapi::run(),
        Command::Worker => worker::run(args).await,
        Command::Web => crate::api::run(args).await,
        Command::Hotlaps { action } => hotlaps::run(args, action).await,
        Command::RaceCalendar { action } => race_calendar::run(args, action).await,
        Command::Vehicles { action } => vehicles::run(args, action).await,
        Command::TrackLocations { action } => track_locations::run(args, action).await,
        Command::Players { action } => players::run(args, action).await,
        Command::Gc(options) => gc::run(args, options).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_consistent() {
        Args::command().debug_assert();
    }
}
