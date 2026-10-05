//! lfspla.net API, replay validator, and CLI.

mod api;
mod auth;
mod cli;
mod jobs;
mod lfs;
mod models;
mod settings;
mod startup;
mod storage;
mod validate;

use clap::Parser;
use tracing_subscriber::EnvFilter;

use cli::{Args, Command};

const DEFAULT_LOG_FILTER: &str = "warn,lfsplanet=info";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| DEFAULT_LOG_FILTER.into()),
        )
        .init();

    let args = Args::parse();
    match &args.command {
        #[cfg(debug_assertions)]
        Command::Demo(options) => cli::demo::run(&args, options).await,
        Command::GenerateConfig(options) => cli::config::run(options),
        Command::Migrate => cli::migrate::run(&args).await,
        Command::Openapi => cli::openapi::run(),
        Command::Worker => cli::worker::run(&args).await,
        Command::Web => api::run(&args).await,
        Command::Hotlap { action } => cli::hotlap::run(&args, action).await,
        Command::Lfs { action } => cli::lfs::run(&args, action).await,
        Command::Era { action } => cli::era::run(&args, action).await,
        Command::Player { action } => cli::player::run(&args, action).await,
        Command::Maintenance { action } => cli::maintenance::run(&args, action).await,
    }
}
