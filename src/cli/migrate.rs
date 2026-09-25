//! Explicit database schema migration command.

use crate::{cli::Args, settings::Settings, startup};

/// Applies pending database schema migrations and exits.
pub(crate) async fn run(args: &Args) -> anyhow::Result<()> {
    let settings = Settings::load(&args.config)?;
    startup::migrate(&settings.database).await
}
