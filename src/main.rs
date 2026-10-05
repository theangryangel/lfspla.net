//! lfspla.net API, replay validator, and CLI.

mod api;
mod cli;
mod db;
mod lfs;
mod models;
mod services;
mod settings;
mod storage;
mod validate;

use clap::Parser;
use cli::Args;
use tracing_subscriber::EnvFilter;
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
    cli::run(&args).await
}
