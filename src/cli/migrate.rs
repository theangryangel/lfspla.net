//! Explicit database schema migration command.

use crate::{cli::Args, db, settings::Settings};

/// Applies pending database schema migrations and exits.
pub(crate) async fn run(args: &Args) -> anyhow::Result<()> {
    let settings = Settings::load(&args.config)?;
    db::migrate(&settings.database).await
}
