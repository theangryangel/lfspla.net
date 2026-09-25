//! Command-line parsing and operator-command dispatch.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

pub(crate) mod config;
pub(crate) mod era;
pub(crate) mod hotlap;
pub(crate) mod lfs;
pub(crate) mod maintenance;
pub(crate) mod migrate;
pub(crate) mod openapi;
pub(crate) mod player;
pub(crate) mod storage;
pub(crate) mod worker;

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
    /// Print a basic YAML configuration to standard output.
    GenerateConfig,
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
    /// Install and maintain named LFS installations.
    Lfs {
        #[command(subcommand)]
        action: lfs::LfsCommand,
    },
    /// Manage database-backed era policy.
    Era {
        #[command(subcommand)]
        action: era::EraCommand,
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
