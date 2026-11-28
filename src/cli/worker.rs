//! Runs the application's background record processors.

use crate::{cli::Args, settings::Settings};
use lfsplanet_core::{db, storage};
use lfsplanet_hotlaps::services::validate_hotlap::HotlapValidation;
use lfsplanet_jobs::{Runner, WorkerConfig};
pub(crate) async fn run(args: &Args) -> anyhow::Result<()> {
    let settings = Settings::load(&args.config)?;
    let database = db::connect(&settings.database, 4).await?;
    let object_store = storage::build(&settings.storage)?;
    let config = WorkerConfig {
        workers: 1,
        poll_interval: settings.hotlaps.validation.poll_interval.duration(),
    };
    let notifications = lfsplanet_hotlaps::services::deliver_webhook::WebhookNotifications::new(
        database.clone(),
        settings.web.public_base_url.url().clone(),
    )?;
    let webhook_config = WorkerConfig {
        workers: settings.hotlaps.webhooks.workers.get(),
        ..WorkerConfig::default()
    };
    let processor = HotlapValidation {
        database,
        object_store,
        runtime: settings.hotlaps.runtime,
        installation_root: settings.hotlaps.installation_root.path().to_owned(),
        settings: settings.hotlaps.validation,
    };
    tracing::info!("background worker started");
    #[cfg(unix)]
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    let mut signal_result = Ok(());
    Runner::new()
        .register(processor, config)?
        .register(notifications, webhook_config)?
        .run(async {
            #[cfg(unix)]
            tokio::select! {
                result = tokio::signal::ctrl_c() => signal_result = result,
                _ = terminate.recv() => {}
            }
            #[cfg(not(unix))]
            {
                signal_result = tokio::signal::ctrl_c().await;
            }
        })
        .await?;
    signal_result?;
    tracing::info!("background worker stopped");
    Ok(())
}
