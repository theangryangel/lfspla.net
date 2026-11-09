//! Game-server process wiring.
use crate::{cli::Args, db, settings::Settings, storage};
pub(crate) async fn run(args: &Args) -> anyhow::Result<()> {
    let settings = Settings::load(&args.config)?;
    let config = settings
        .game_server
        .ok_or_else(|| anyhow::anyhow!("game_server configuration is required"))?;
    let database = db::connect(&settings.database, 5).await?;
    let store = storage::build(&settings.storage)?;
    crate::services::game_server::run(database, store, config).await
}
